//! Setting value kinds — minimal metadata for UI rendering and validation.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "values")]
pub enum Kind {
    Bool,
    U64,
    F64,
    String,
    Path,
    Url,
    Enum(&'static [&'static str]),
}

impl Kind {
    /// Validate a value against this kind. Returns the canonical string form
    /// on success, or a human-readable error.
    pub fn parse(&self, value: &str) -> Result<String, String> {
        let v = value.trim();
        match self {
            Kind::Bool => match v.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Ok("true".to_string()),
                "false" | "0" | "no" | "off" => Ok("false".to_string()),
                _ => Err(format!("expected boolean (true/false), got '{value}'")),
            },
            Kind::U64 => v
                .parse::<u64>()
                .map(|n| n.to_string())
                .map_err(|_| format!("expected u64, got '{value}'")),
            Kind::F64 => v
                .parse::<f64>()
                .map(|n| n.to_string())
                .map_err(|_| format!("expected f64, got '{value}'")),
            Kind::String => Ok(v.to_string()),
            Kind::Path => {
                if v.is_empty() {
                    Err("expected non-empty path".to_string())
                } else {
                    Ok(v.to_string())
                }
            }
            Kind::Url => {
                if v.contains("://") {
                    Ok(v.to_string())
                } else {
                    Err(format!("expected URL with scheme, got '{value}'"))
                }
            }
            Kind::Enum(allowed) => {
                if allowed.contains(&v) {
                    Ok(v.to_string())
                } else {
                    Err(format!("expected one of {allowed:?}, got '{value}'"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_accepts_truthy_and_falsy_synonyms() {
        for v in ["true", "1", "yes", "on", "TRUE", " on "] {
            assert_eq!(Kind::Bool.parse(v).unwrap(), "true", "input: {v:?}");
        }
        for v in ["false", "0", "no", "off", "FALSE"] {
            assert_eq!(Kind::Bool.parse(v).unwrap(), "false", "input: {v:?}");
        }
    }

    #[test]
    fn bool_rejects_garbage_and_quotes_input_in_error() {
        let err = Kind::Bool.parse("maybe").unwrap_err();
        assert!(err.contains("'maybe'"), "got: {err}");
        assert!(err.to_lowercase().contains("bool"));
    }

    #[test]
    fn u64_parses_and_rejects() {
        assert_eq!(Kind::U64.parse("0").unwrap(), "0");
        assert_eq!(
            Kind::U64.parse("18446744073709551615").unwrap(),
            "18446744073709551615"
        );
        let err = Kind::U64.parse("fast").unwrap_err();
        assert!(err.contains("u64") && err.contains("'fast'"), "got: {err}");
        // Negative number is invalid for u64.
        assert!(Kind::U64.parse("-1").is_err());
    }

    #[test]
    fn f64_parses_and_rejects() {
        assert!(Kind::F64.parse("3.14").is_ok());
        assert!(Kind::F64.parse("0").is_ok());
        let err = Kind::F64.parse("pi").unwrap_err();
        assert!(err.contains("f64"), "got: {err}");
    }

    #[test]
    fn enum_allows_only_listed_values() {
        let kind = Kind::Enum(&["fixed", "lightweight", "semantic"]);
        assert_eq!(kind.parse("semantic").unwrap(), "semantic");
        let err = kind.parse("banana").unwrap_err();
        assert!(
            err.contains("'banana'") && err.contains("fixed"),
            "got: {err}"
        );
    }

    #[test]
    fn url_requires_scheme() {
        assert!(Kind::Url.parse("redis://localhost").is_ok());
        assert!(Kind::Url.parse("http://example.com").is_ok());
        let err = Kind::Url.parse("localhost").unwrap_err();
        assert!(err.contains("URL") || err.contains("scheme"), "got: {err}");
    }

    #[test]
    fn path_rejects_empty_but_not_relative() {
        assert!(Kind::Path.parse("/tmp/x").is_ok());
        assert!(Kind::Path.parse("relative/path").is_ok());
        assert!(Kind::Path.parse("").is_err());
        assert!(Kind::Path.parse("   ").is_err()); // trimmed empty
    }

    #[test]
    fn string_trims_and_passes_through() {
        assert_eq!(Kind::String.parse("  hello  ").unwrap(), "hello");
        assert_eq!(
            Kind::String.parse("anything goes").unwrap(),
            "anything goes"
        );
    }
}
