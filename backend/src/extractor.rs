// src/extractor.rs — External document extractor trait and registry.
//
// The built-in pipeline (extract_ir_from_bytes in index.rs) handles Markdown,
// HTML, and code structurally; for formats like PDF and DOCX it falls back to
// flat text.  External extractors (Docling, Unstructured, …) provide richer
// structural extraction for those formats via HTTP sidecars.
//
// At startup main.rs calls init_registry() once.  During indexing, extract_ir()
// consults the registry before falling through to the built-in path.

use crate::doc_ir::{BoundingBox, DocBlock, DocIR};
use crate::mime_detect::ContentType;
use std::sync::OnceLock;
use tracing::{debug, warn};

// ── Trait ─────────────────────────────────────────────────────────────────────

pub trait DocExtractor: Send + Sync {
    fn name(&self) -> &str;
    /// Return true when this extractor can improve on built-in for this type.
    fn can_handle(&self, content_type: &ContentType) -> bool;
    /// Takes ownership of `bytes` so HTTP extractors can pass them directly to
    /// the multipart body without an extra allocation.
    fn extract(&self, bytes: Vec<u8>, filename: &str, ct: &ContentType) -> anyhow::Result<DocIR>;
}

// ── Registry ──────────────────────────────────────────────────────────────────

pub struct ExtractorRegistry {
    extractors: Vec<Box<dyn DocExtractor>>,
}

impl ExtractorRegistry {
    pub fn new(extractors: Vec<Box<dyn DocExtractor>>) -> Self {
        Self { extractors }
    }

    /// True if any registered extractor claims this content type.
    pub fn has_handler(&self, ct: &ContentType) -> bool {
        self.extractors.iter().any(|e| e.can_handle(ct))
    }

    /// Try each extractor in priority order; return the first success.
    /// Bytes are moved into the first matching extractor; cloned only on retry.
    pub fn extract(&self, bytes: Vec<u8>, filename: &str, ct: &ContentType) -> Option<DocIR> {
        let matching: Vec<_> = self
            .extractors
            .iter()
            .filter(|e| e.can_handle(ct))
            .collect();
        let n = matching.len();
        let mut bytes_opt = Some(bytes);
        for (i, ext) in matching.iter().enumerate() {
            let b = if i + 1 < n {
                bytes_opt.as_ref().unwrap().clone()
            } else {
                bytes_opt.take().unwrap()
            };
            match ext.extract(b, filename, ct) {
                Ok(ir) => {
                    debug!(
                        extractor = ext.name(),
                        filename, "external extraction succeeded"
                    );
                    return Some(ir);
                }
                Err(e) => {
                    warn!(extractor = ext.name(), filename, error = %e, "extractor failed, trying next");
                }
            }
        }
        None
    }
}

static REGISTRY: OnceLock<ExtractorRegistry> = OnceLock::new();

pub fn init_registry(extractors: Vec<Box<dyn DocExtractor>>) {
    if REGISTRY.set(ExtractorRegistry::new(extractors)).is_err() {
        warn!("extractor registry was already initialized; ignoring second init");
    }
}

pub fn global_registry() -> Option<&'static ExtractorRegistry> {
    REGISTRY.get()
}

// ── Docling extractor ─────────────────────────────────────────────────────────
//
// The Docling sidecar (`docling_sidecar/`) is a FastAPI Python service that
// calls the Docling library and returns DocIR-shaped JSON.  This extractor
// handles PDF, DOCX, PPTX, ODT, XLSX, EPUB — formats where Docling's layout
// analysis gives much richer structure than the built-in XML/text stripper.

pub struct DoclingExtractor {
    client: reqwest::blocking::Client,
    endpoint: String,
}

impl DoclingExtractor {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("failed to build reqwest blocking client"),
            endpoint: endpoint.into(),
        }
    }

    /// Probe the sidecar health endpoint; call this at startup before registering.
    pub fn health_check(&self) -> anyhow::Result<()> {
        let url = format!("{}/health", self.endpoint);
        let resp = self.client.get(&url).send()?;
        anyhow::ensure!(
            resp.status().is_success(),
            "docling sidecar unhealthy (status {})",
            resp.status()
        );
        Ok(())
    }
}

impl DocExtractor for DoclingExtractor {
    fn name(&self) -> &str {
        "docling"
    }

    fn can_handle(&self, ct: &ContentType) -> bool {
        // Only claim PDFs — DOCX/EPUB/PPTX are now handled structurally in Rust.
        // Docling adds value for scanned PDFs where layout AI detects reading order
        // and table boundaries that aren't encoded in the file format itself.
        matches!(ct, ContentType::Pdf)
    }

    fn extract(&self, bytes: Vec<u8>, filename: &str, _ct: &ContentType) -> anyhow::Result<DocIR> {
        let url = format!("{}/convert", self.endpoint);
        let part = reqwest::blocking::multipart::Part::bytes(bytes)
            .file_name(filename.to_string())
            .mime_str("application/octet-stream")?;
        let form = reqwest::blocking::multipart::Form::new().part("file", part);
        let resp = self.client.post(&url).multipart(form).send()?;
        let status = resp.status();
        anyhow::ensure!(
            status.is_success(),
            "docling sidecar returned {}: {}",
            status,
            resp.text().unwrap_or_default()
        );
        let mut ir: DocIR = resp.json()?;
        ir.tag_extractor("docling");
        Ok(ir)
    }
}

// ── Unstructured extractor ────────────────────────────────────────────────────
//
// Wraps the Unstructured self-hosted API server (or cloud API) and maps its
// typed element stream to DocIR.  The server image is:
//   downloads.unstructured.io/unstructured-io/unstructured-api:latest
// Port 8000, profile "unstructured" in docker-compose.

#[derive(serde::Deserialize)]
struct UnstrElement {
    #[serde(rename = "type")]
    element_type: String,
    text: String,
    metadata: Option<UnstrMeta>,
}

#[derive(serde::Deserialize)]
struct UnstrMeta {
    page_number: Option<u32>,
    coordinates: Option<UnstrCoords>,
}

#[derive(serde::Deserialize)]
struct UnstrCoords {
    points: Option<Vec<Vec<f32>>>,
}

pub struct UnstructuredExtractor {
    client: reqwest::blocking::Client,
    endpoint: String,
    api_key: Option<String>,
}

impl UnstructuredExtractor {
    pub fn new(endpoint: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("failed to build reqwest blocking client"),
            endpoint: endpoint.into(),
            api_key,
        }
    }

    pub fn health_check(&self) -> anyhow::Result<()> {
        let url = format!("{}/healthcheck", self.endpoint);
        let resp = self.client.get(&url).send()?;
        anyhow::ensure!(
            resp.status().is_success(),
            "unstructured sidecar unhealthy (status {})",
            resp.status()
        );
        Ok(())
    }
}

impl DocExtractor for UnstructuredExtractor {
    fn name(&self) -> &str {
        "unstructured"
    }

    fn can_handle(&self, ct: &ContentType) -> bool {
        // PDF only — DOCX/EPUB/PPTX are handled structurally in Rust.
        matches!(ct, ContentType::Pdf)
    }

    fn extract(&self, bytes: Vec<u8>, filename: &str, _ct: &ContentType) -> anyhow::Result<DocIR> {
        use crate::doc_ir::BlockType;

        let url = format!("{}/general/v0/general", self.endpoint);
        let part = reqwest::blocking::multipart::Part::bytes(bytes)
            .file_name(filename.to_string())
            .mime_str("application/octet-stream")?;
        let form = reqwest::blocking::multipart::Form::new()
            .part("files", part)
            .text("strategy", "auto");

        let mut req = self.client.post(&url).multipart(form);
        if let Some(ref key) = self.api_key {
            req = req.header("unstructured-api-key", key);
        }

        let resp = req.send()?;
        let status = resp.status();
        anyhow::ensure!(
            status.is_success(),
            "unstructured returned {}: {}",
            status,
            resp.text().unwrap_or_default()
        );

        let elements: Vec<UnstrElement> = resp.json()?;
        let mut ir = DocIR::new(filename, "pdf");

        for el in elements {
            let text = el.text.clone();
            let page = el.metadata.as_ref().and_then(|m| m.page_number);
            let bbox = el
                .metadata
                .as_ref()
                .and_then(|m| m.coordinates.as_ref())
                .and_then(|c| c.points.as_ref())
                .filter(|pts| pts.len() >= 4)
                .map(|pts| BoundingBox {
                    page: page.unwrap_or(1),
                    x0: pts[0].first().copied().unwrap_or(0.0),
                    y0: pts[0].get(1).copied().unwrap_or(0.0),
                    x1: pts[2].first().copied().unwrap_or(0.0),
                    y1: pts[2].get(1).copied().unwrap_or(0.0),
                });

            let mut block = match el.element_type.as_str() {
                "Title" => DocBlock::header(1, text),
                "SectionHeader" | "Header" => DocBlock::header(2, text),
                "Table" => DocBlock::table(0, 0, text),
                "CodeSnippet" | "Code" => DocBlock::code(None, text),
                "ListItem" => {
                    let mut b = DocBlock::text(text);
                    b.block_type = BlockType::List { ordered: false };
                    b
                }
                "Image" | "Figure" => {
                    let mut b = DocBlock::text(String::new());
                    b.block_type = BlockType::Image { alt: Some(el.text) };
                    b
                }
                "FigureCaption" => {
                    let mut b = DocBlock::text(text);
                    b.block_type = BlockType::Caption;
                    b
                }
                "PageBreak" => {
                    let mut b = DocBlock::text(String::new());
                    b.block_type = BlockType::PageBreak;
                    b
                }
                _ => DocBlock::text(text),
            };
            block.page = page;
            block.bbox = bbox;
            ir.push(block);
        }

        ir.tag_extractor("unstructured");
        Ok(ir)
    }
}

// ── Multi-extractor Fusion ────────────────────────────────────────────────────
//
// FusionExtractor wraps two or more DocExtractors and merges their outputs.
// It only claims a content type when ≥2 sources can handle it, so single-source
// formats continue to fall through to the built-in path unchanged.

#[derive(Debug, Clone, Copy)]
pub enum FusionStrategy {
    /// Highest-confidence extractor wins; others fill spatial gaps not covered by it.
    Waterfall,
    /// All blocks compete position by position; highest-confidence block per cluster wins.
    BestBlock,
}

pub struct WeightedExtractor {
    pub extractor: Box<dyn DocExtractor>,
    /// 0.0–1.0 — higher = preferred when two blocks occupy the same position.
    pub confidence: f32,
}

pub struct FusionExtractor {
    sources: Vec<WeightedExtractor>,
    strategy: FusionStrategy,
}

impl FusionExtractor {
    pub fn new(sources: Vec<WeightedExtractor>, strategy: FusionStrategy) -> Self {
        Self { sources, strategy }
    }
}

impl DocExtractor for FusionExtractor {
    fn name(&self) -> &str {
        "fusion"
    }

    /// Only claim formats where ≥2 sources compete; single-source formats fall through.
    fn can_handle(&self, ct: &ContentType) -> bool {
        self.sources
            .iter()
            .filter(|w| w.extractor.can_handle(ct))
            .count()
            >= 2
    }

    fn extract(&self, bytes: Vec<u8>, filename: &str, ct: &ContentType) -> anyhow::Result<DocIR> {
        let matching: Vec<_> = self
            .sources
            .iter()
            .filter(|w| w.extractor.can_handle(ct))
            .collect();
        let n = matching.len();
        let mut bytes_opt = Some(bytes);
        let mut results: Vec<(DocIR, f32)> = Vec::new();

        for (i, w) in matching.iter().enumerate() {
            let b = if i + 1 < n {
                bytes_opt.as_ref().unwrap().clone()
            } else {
                bytes_opt.take().unwrap()
            };
            match w.extractor.extract(b, filename, ct) {
                Ok(mut ir) => {
                    let conf_str = w.confidence.to_string();
                    let name = w.extractor.name().to_string();
                    for b in &mut ir.blocks {
                        b.metadata.insert("extractor".into(), name.clone());
                        b.metadata.insert("confidence".into(), conf_str.clone());
                    }
                    results.push((ir, w.confidence));
                }
                Err(e) => {
                    warn!(extractor = w.extractor.name(), %e, "fusion source failed, skipping");
                }
            }
        }

        match results.len() {
            0 => anyhow::bail!("all fusion sources failed for '{}'", filename),
            1 => Ok(results.remove(0).0),
            _ => Ok(fuse(results, self.strategy)),
        }
    }
}

fn fuse(mut results: Vec<(DocIR, f32)>, strategy: FusionStrategy) -> DocIR {
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    match strategy {
        FusionStrategy::Waterfall => {
            let (mut primary, _) = results.remove(0);
            for (secondary, _) in results {
                for block in secondary.blocks {
                    if !primary.blocks.iter().any(|p| blocks_conflict(p, &block)) {
                        primary.push(block);
                    }
                }
            }
            sort_by_position(&mut primary.blocks);
            primary
        }
        FusionStrategy::BestBlock => {
            let source = results[0].0.source.clone();
            let ct = results[0].0.content_type.clone();

            let mut candidates: Vec<(DocBlock, f32)> = results
                .into_iter()
                .flat_map(|(ir, _)| {
                    ir.blocks.into_iter().map(|b| {
                        let conf = b
                            .metadata
                            .get("confidence")
                            .and_then(|s| s.parse::<f32>().ok())
                            .unwrap_or(0.5);
                        (b, conf)
                    })
                })
                .collect();

            candidates.sort_by_key(|(b, _)| {
                (
                    b.page.unwrap_or(u32::MAX),
                    b.bbox
                        .as_ref()
                        .map(|bb| (bb.y0 * 1000.0) as i64)
                        .unwrap_or(i64::MAX),
                )
            });

            // Greedy: keep a block unless a conflicting higher-confidence block already won.
            let mut kept: Vec<(DocBlock, f32)> = Vec::new();
            for (block, conf) in candidates {
                if let Some(pos) = kept.iter().position(|(k, _)| blocks_conflict(k, &block)) {
                    if conf > kept[pos].1 {
                        kept[pos] = (block, conf);
                    }
                } else {
                    kept.push((block, conf));
                }
            }

            let mut ir = DocIR::new(source, ct);
            for (b, _) in kept {
                ir.push(b);
            }
            ir
        }
    }
}

fn sort_by_position(blocks: &mut [DocBlock]) {
    blocks.sort_by_key(|b| {
        (
            b.page.unwrap_or(u32::MAX),
            b.bbox
                .as_ref()
                .map(|bb| (bb.y0 * 1000.0) as i64)
                .unwrap_or(i64::MAX),
        )
    });
}

fn blocks_conflict(a: &DocBlock, b: &DocBlock) -> bool {
    if let (Some(ba), Some(bb)) = (&a.bbox, &b.bbox) {
        if ba.page != bb.page {
            return false;
        }
        let overlap_top = ba.y0.max(bb.y0);
        let overlap_bot = ba.y1.min(bb.y1);
        if overlap_top >= overlap_bot {
            return false;
        }
        let overlap_h = overlap_bot - overlap_top;
        let min_h = (ba.y1 - ba.y0).min(bb.y1 - bb.y0);
        return min_h > 0.0 && overlap_h / min_h > 0.5;
    }
    // No bboxes: fall back to text similarity
    jaccard(&a.text, &b.text) > 0.7
}

fn jaccard(a: &str, b: &str) -> f32 {
    use std::collections::HashSet;
    let wa: HashSet<&str> = a.split_whitespace().collect();
    let wb: HashSet<&str> = b.split_whitespace().collect();
    let union = wa.union(&wb).count();
    if union == 0 {
        return if a.is_empty() && b.is_empty() {
            1.0
        } else {
            0.0
        };
    }
    wa.intersection(&wb).count() as f32 / union as f32
}
