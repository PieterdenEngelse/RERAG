// src/mime_detect.rs
//
// MIME type detection using file magic bytes with fallback to extension-based detection.
// This provides more reliable file type identification than extension-only approaches.

use std::path::Path;
use tracing::debug;

/// Detected content type for chunking strategy selection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    Pdf,
    Text,
    Markdown,
    Html,
    Code(CodeLanguage),
    Json,
    Xml,
    Binary,
    Unknown,
}

/// Programming language for code-specific chunking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    Cpp,
    C,
    Ruby,
    Php,
    Shell,
    Sql,
    Yaml,
    Toml,
    Other,
}

impl ContentType {
    /// Check if this content type is text-based (can be chunked as text)
    pub fn is_text_based(&self) -> bool {
        !matches!(self, ContentType::Binary | ContentType::Unknown)
    }

    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            ContentType::Pdf => "PDF document",
            ContentType::Text => "Plain text",
            ContentType::Markdown => "Markdown",
            ContentType::Html => "HTML",
            ContentType::Code(_) => "Source code",
            ContentType::Json => "JSON",
            ContentType::Xml => "XML",
            ContentType::Binary => "Binary file",
            ContentType::Unknown => "Unknown",
        }
    }
}

/// Detect content type from file bytes using magic byte inspection.
/// Falls back to extension-based detection if magic bytes don't match.
pub fn detect_content_type(bytes: &[u8], filename: Option<&str>) -> ContentType {
    // First, try magic byte detection
    if let Some(kind) = infer::get(bytes) {
        let mime = kind.mime_type();
        debug!("MIME detected from magic bytes: {}", mime);

        let content_type = match mime {
            "application/pdf" => ContentType::Pdf,
            "text/html" => ContentType::Html,
            "text/xml" | "application/xml" => ContentType::Xml,
            "application/json" => ContentType::Json,
            // infer doesn't detect plain text well, fall through to extension check
            _ if mime.starts_with("text/") => ContentType::Text,
            _ if mime.starts_with("image/")
                || mime.starts_with("audio/")
                || mime.starts_with("video/") =>
            {
                ContentType::Binary
            }
            _ => ContentType::Unknown,
        };

        // If we got a definitive match (not Unknown), return it
        if content_type != ContentType::Unknown {
            return content_type;
        }
    }

    // Fall back to extension-based detection
    if let Some(name) = filename {
        let ext_type = detect_from_extension(name);
        if ext_type != ContentType::Unknown {
            debug!("Content type detected from extension: {:?}", ext_type);
            return ext_type;
        }
    }

    // Final fallback: check if it looks like text
    if is_likely_text(bytes) {
        debug!("Content appears to be text based on byte analysis");
        ContentType::Text
    } else {
        debug!("Content type unknown, treating as binary");
        ContentType::Binary
    }
}

/// Detect content type from file extension
pub fn detect_from_extension(filename: &str) -> ContentType {
    let path = Path::new(filename);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        // Documents
        Some("pdf") => ContentType::Pdf,
        Some("txt") | Some("text") => ContentType::Text,
        Some("md") | Some("markdown") => ContentType::Markdown,
        Some("html") | Some("htm") => ContentType::Html,
        Some("xml") | Some("xhtml") => ContentType::Xml,
        Some("json") => ContentType::Json,

        // Code files
        Some("rs") => ContentType::Code(CodeLanguage::Rust),
        Some("py") | Some("pyw") => ContentType::Code(CodeLanguage::Python),
        Some("js") | Some("mjs") | Some("cjs") => ContentType::Code(CodeLanguage::JavaScript),
        Some("ts") | Some("tsx") => ContentType::Code(CodeLanguage::TypeScript),
        Some("go") => ContentType::Code(CodeLanguage::Go),
        Some("java") => ContentType::Code(CodeLanguage::Java),
        Some("cs") => ContentType::Code(CodeLanguage::CSharp),
        Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") => {
            ContentType::Code(CodeLanguage::Cpp)
        }
        Some("c") | Some("h") => ContentType::Code(CodeLanguage::C),
        Some("rb") => ContentType::Code(CodeLanguage::Ruby),
        Some("php") => ContentType::Code(CodeLanguage::Php),
        Some("sh") | Some("bash") | Some("zsh") => ContentType::Code(CodeLanguage::Shell),
        Some("sql") => ContentType::Code(CodeLanguage::Sql),
        Some("yaml") | Some("yml") => ContentType::Code(CodeLanguage::Yaml),
        Some("toml") => ContentType::Code(CodeLanguage::Toml),

        // Config files often without extension but with specific names
        _ => {
            // Check for common config file names
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            match name.to_lowercase().as_str() {
                "dockerfile" | "makefile" | "rakefile" | "gemfile" => {
                    ContentType::Code(CodeLanguage::Other)
                }
                ".gitignore" | ".dockerignore" | ".env" => ContentType::Text,
                "readme" | "license" | "changelog" | "authors" => ContentType::Text,
                _ => ContentType::Unknown,
            }
        }
    }
}

/// Check if bytes appear to be text content (UTF-8 or ASCII)
fn is_likely_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }

    // Check first 8KB for text characteristics
    let sample = &bytes[..bytes.len().min(8192)];

    // Count non-text bytes
    let non_text_count = sample
        .iter()
        .filter(|&&b| {
            // Allow common text bytes: printable ASCII, newlines, tabs, UTF-8 continuation bytes
            !(b == 0x09 // tab
                || b == 0x0A // newline
                || b == 0x0D // carriage return
                || (0x20..=0x7E).contains(&b) // printable ASCII
                || (0x80..=0xBF).contains(&b) // UTF-8 continuation
                || (0xC0..=0xF7).contains(&b)) // UTF-8 start bytes
        })
        .count();

    // If less than 5% non-text bytes, consider it text
    let threshold = sample.len() / 20;
    non_text_count <= threshold
}

/// Detect content type from a file path (reads file and detects)
pub fn detect_from_file(path: &Path) -> Result<ContentType, std::io::Error> {
    let bytes = std::fs::read(path)?;
    let filename = path.file_name().and_then(|n| n.to_str());
    Ok(detect_content_type(&bytes, filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_detection() {
        assert_eq!(detect_from_extension("test.pdf"), ContentType::Pdf);
        assert_eq!(detect_from_extension("test.txt"), ContentType::Text);
        assert_eq!(detect_from_extension("test.md"), ContentType::Markdown);
        assert_eq!(detect_from_extension("test.html"), ContentType::Html);
        assert_eq!(
            detect_from_extension("test.rs"),
            ContentType::Code(CodeLanguage::Rust)
        );
        assert_eq!(
            detect_from_extension("test.py"),
            ContentType::Code(CodeLanguage::Python)
        );
    }

    #[test]
    fn test_text_detection() {
        let text = b"Hello, this is plain text content.\nWith multiple lines.";
        assert!(is_likely_text(text));

        let binary = [0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE];
        assert!(!is_likely_text(&binary));
    }

    #[test]
    fn test_content_type_from_bytes() {
        // Plain text
        let text = b"This is plain text content.";
        let ct = detect_content_type(text, Some("test.txt"));
        assert!(ct.is_text_based());

        // PDF magic bytes
        let pdf = b"%PDF-1.4 fake pdf content";
        let ct = detect_content_type(pdf, Some("test.pdf"));
        assert_eq!(ct, ContentType::Pdf);
    }

    #[test]
    fn test_special_filenames() {
        assert_eq!(
            detect_from_extension("Dockerfile"),
            ContentType::Code(CodeLanguage::Other)
        );
        assert_eq!(detect_from_extension("README"), ContentType::Text);
        assert_eq!(detect_from_extension(".gitignore"), ContentType::Text);
    }
}
