use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::RwLock;

const MAX_RECENT_FILES: usize = 1000;

#[derive(Clone, Serialize, Default)]
pub struct FormatCounts {
    pub ok: u64,
    pub empty: u64,
    pub chars: u64,
}

#[derive(Clone, Serialize, Default)]
pub struct OcrCounts {
    pub attempted: u64,
    pub ok: u64,
    pub no_text: u64,
    pub no_pages: u64,
    pub unavailable: u64,
}

#[derive(Clone, Serialize, Default)]
pub struct FileRecord {
    pub filename: String,
    pub path: String,
    pub format: String,
    pub ok: bool,
    pub chars: u64,
    pub corpus: String,
}

#[derive(Clone, Serialize, Default)]
pub struct ExtractionStats {
    pub by_format: HashMap<String, FormatCounts>,
    pub ocr: OcrCounts,
    pub recent_files: Vec<FileRecord>,
}

static STATS: Lazy<RwLock<ExtractionStats>> = Lazy::new(|| RwLock::new(ExtractionStats::default()));

/// Called at startup to seed in-memory stats from SQLite history.
pub fn load_history() {
    let records = crate::db::extraction_records::load_recent();
    if let Ok(mut s) = STATS.write() {
        for rec in &records {
            let entry = s.by_format.entry(rec.format.clone()).or_default();
            if rec.ok {
                entry.ok += 1;
                entry.chars += rec.chars;
            } else {
                entry.empty += 1;
            }
        }
        // load_recent returns newest-first; keep that order
        s.recent_files = records;
    }
}

pub fn record_format(
    format: &str,
    ok: bool,
    chars: usize,
    filename: &str,
    path: &str,
    corpus: &str,
) {
    let rec = FileRecord {
        filename: filename.to_string(),
        path: path.to_string(),
        format: format.to_string(),
        ok,
        chars: chars as u64,
        corpus: corpus.to_string(),
    };

    // Persist to SQLite
    crate::db::extraction_records::insert(&rec);

    // Update in-memory state
    if let Ok(mut s) = STATS.write() {
        let entry = s.by_format.entry(format.to_string()).or_default();
        if ok {
            entry.ok += 1;
            entry.chars += chars as u64;
        } else {
            entry.empty += 1;
        }
        if s.recent_files.len() >= MAX_RECENT_FILES {
            s.recent_files.pop(); // newest-first: drop oldest at tail
        }
        s.recent_files.insert(0, rec);
    }
}

pub fn forget_file(filename: &str) {
    crate::db::extraction_records::delete_by_filename(filename);
    if let Ok(mut s) = STATS.write() {
        s.recent_files.retain(|r| r.filename != filename);
    }
}

pub fn record_ocr_attempted() {
    if let Ok(mut s) = STATS.write() {
        s.ocr.attempted += 1;
    }
}

pub fn record_ocr_ok() {
    if let Ok(mut s) = STATS.write() {
        s.ocr.ok += 1;
    }
}

pub fn record_ocr_no_text() {
    if let Ok(mut s) = STATS.write() {
        s.ocr.no_text += 1;
    }
}

pub fn record_ocr_no_pages() {
    if let Ok(mut s) = STATS.write() {
        s.ocr.no_pages += 1;
    }
}

pub fn record_ocr_unavailable() {
    if let Ok(mut s) = STATS.write() {
        s.ocr.unavailable += 1;
    }
}

pub fn get_stats() -> ExtractionStats {
    get_stats_for(None)
}

pub fn get_stats_for(corpus: Option<&str>) -> ExtractionStats {
    let corpus = corpus.filter(|s| !s.is_empty());
    let snapshot = STATS.read().map(|s| s.clone()).unwrap_or_default();
    let Some(slug) = corpus else {
        return snapshot;
    };
    let recent_files: Vec<FileRecord> = snapshot
        .recent_files
        .into_iter()
        .filter(|r| r.corpus == slug)
        .collect();
    let mut by_format: std::collections::HashMap<String, FormatCounts> =
        std::collections::HashMap::new();
    for rec in &recent_files {
        let entry = by_format.entry(rec.format.clone()).or_default();
        if rec.ok {
            entry.ok += 1;
            entry.chars += rec.chars;
        } else {
            entry.empty += 1;
        }
    }
    ExtractionStats {
        by_format,
        ocr: snapshot.ocr,
        recent_files,
    }
}
