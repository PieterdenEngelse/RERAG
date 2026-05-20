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
