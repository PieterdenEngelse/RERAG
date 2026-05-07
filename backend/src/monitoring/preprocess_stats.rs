use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

const MAX_RECENT: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreprocessFileRecord {
    pub filename: String,
    pub kind: String, // "html" | "unicode" | "passthrough"
    pub chars_in: u64,
    pub chars_out: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PreprocessStats {
    pub html_files: u64,
    pub html_chars_in: u64,
    pub html_chars_out: u64,
    pub html_tags_stripped: u64,
    pub unicode_files: u64,
    pub unicode_chars_in: u64,
    pub unicode_chars_out: u64,
    pub passthrough_files: u64,
    pub passthrough_chars: u64,
    #[serde(default)]
    pub recent_files: Vec<PreprocessFileRecord>,
}

impl PreprocessStats {
    fn push_file(&mut self, rec: PreprocessFileRecord) {
        self.recent_files.push(rec);
        if self.recent_files.len() > MAX_RECENT {
            self.recent_files.remove(0);
        }
    }
}

/// Keyed by corpus slug; "_all" holds the aggregate across all corpora.
static STATE: Lazy<RwLock<HashMap<String, PreprocessStats>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

static SNAPSHOT_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init(path: PathBuf) {
    let _ = SNAPSHOT_PATH.set(path.clone());
    if let Ok(data) = std::fs::read_to_string(&path) {
        // Try new format (HashMap) first, then fall back to legacy single-struct.
        if let Ok(map) = serde_json::from_str::<HashMap<String, PreprocessStats>>(&data) {
            if let Ok(mut s) = STATE.write() {
                *s = map;
            }
        } else if let Ok(legacy) = serde_json::from_str::<PreprocessStats>(&data) {
            if let Ok(mut s) = STATE.write() {
                s.insert("_all".to_string(), legacy);
            }
        }
    }
}

pub fn flush() {
    let Some(path) = SNAPSHOT_PATH.get() else {
        return;
    };
    let snapshot = STATE.read().map(|s| s.clone()).unwrap_or_default();
    if let Ok(json) = serde_json::to_string(&snapshot) {
        let _ = std::fs::write(path, json);
    }
}

pub fn record_html(
    filename: &str,
    corpus: &str,
    chars_in: usize,
    chars_out: usize,
    tags_stripped: u64,
) {
    let rec = PreprocessFileRecord {
        filename: filename.to_string(),
        kind: "html".to_string(),
        chars_in: chars_in as u64,
        chars_out: chars_out as u64,
    };
    if let Ok(mut map) = STATE.write() {
        let e = map.entry(corpus.to_string()).or_default();
        e.html_files += 1;
        e.html_chars_in += chars_in as u64;
        e.html_chars_out += chars_out as u64;
        e.html_tags_stripped += tags_stripped;
        e.push_file(rec.clone());
        if corpus != "_all" {
            let all = map.entry("_all".to_string()).or_default();
            all.html_files += 1;
            all.html_chars_in += chars_in as u64;
            all.html_chars_out += chars_out as u64;
            all.html_tags_stripped += tags_stripped;
            all.push_file(rec);
        }
    }
}

pub fn record_unicode(filename: &str, corpus: &str, chars_in: usize, chars_out: usize) {
    let rec = PreprocessFileRecord {
        filename: filename.to_string(),
        kind: "unicode".to_string(),
        chars_in: chars_in as u64,
        chars_out: chars_out as u64,
    };
    if let Ok(mut map) = STATE.write() {
        let e = map.entry(corpus.to_string()).or_default();
        e.unicode_files += 1;
        e.unicode_chars_in += chars_in as u64;
        e.unicode_chars_out += chars_out as u64;
        e.push_file(rec.clone());
        if corpus != "_all" {
            let all = map.entry("_all".to_string()).or_default();
            all.unicode_files += 1;
            all.unicode_chars_in += chars_in as u64;
            all.unicode_chars_out += chars_out as u64;
            all.push_file(rec);
        }
    }
}

pub fn record_passthrough(filename: &str, corpus: &str, chars: usize) {
    let rec = PreprocessFileRecord {
        filename: filename.to_string(),
        kind: "passthrough".to_string(),
        chars_in: chars as u64,
        chars_out: chars as u64,
    };
    if let Ok(mut map) = STATE.write() {
        let e = map.entry(corpus.to_string()).or_default();
        e.passthrough_files += 1;
        e.passthrough_chars += chars as u64;
        e.push_file(rec.clone());
        if corpus != "_all" {
            let all = map.entry("_all".to_string()).or_default();
            all.passthrough_files += 1;
            all.passthrough_chars += chars as u64;
            all.push_file(rec);
        }
    }
}

pub fn get_stats(corpus: Option<&str>) -> PreprocessStats {
    let key = corpus.filter(|s| !s.is_empty()).unwrap_or("_all");
    STATE
        .read()
        .map(|map| map.get(key).cloned().unwrap_or_default())
        .unwrap_or_default()
}
