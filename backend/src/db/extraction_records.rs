use crate::monitoring::FileRecord;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tracing::warn;

const MAX_RECORDS: usize = 1000;
const DAYS_HISTORY: u64 = 7;

static DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static DB_CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

pub fn init(db_path: PathBuf) {
    let _ = DB_PATH.set(db_path.clone());
    match Connection::open(&db_path) {
        Ok(conn) => {
            let _ = DB_CONN.set(Mutex::new(conn));
        }
        Err(e) => warn!("extraction_records: failed to open DB: {}", e),
    }
}

pub fn insert(rec: &FileRecord) {
    let Some(mutex) = DB_CONN.get() else { return };
    let Ok(conn) = mutex.lock() else { return };
    if let Err(e) = conn.execute(
        "INSERT INTO extraction_records (filename, path, format, ok, chars) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![rec.filename, rec.path, rec.format, rec.ok as i64, rec.chars as i64],
    ) {
        warn!("extraction_records: insert failed: {}", e);
        return;
    }
    // Prune: keep only last MAX_RECORDS rows and drop anything older than DAYS_HISTORY days
    let cutoff = format!("-{} days", DAYS_HISTORY);
    let _ = conn.execute(
        "DELETE FROM extraction_records WHERE recorded_at < datetime('now', ?1)",
        params![cutoff],
    );
    let _ = conn.execute(
        "DELETE FROM extraction_records WHERE id NOT IN (
            SELECT id FROM extraction_records ORDER BY recorded_at DESC LIMIT ?1
        )",
        params![MAX_RECORDS as i64],
    );
}

pub fn load_recent() -> Vec<FileRecord> {
    let Some(mutex) = DB_CONN.get() else {
        return vec![];
    };
    let Ok(conn) = mutex.lock() else {
        return vec![];
    };
    let cutoff = format!("-{} days", DAYS_HISTORY);
    let mut stmt = match conn.prepare(
        "SELECT filename, path, format, ok, chars FROM extraction_records
         WHERE recorded_at > datetime('now', ?1)
         ORDER BY recorded_at DESC LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!("extraction_records: prepare failed: {}", e);
            return vec![];
        }
    };
    stmt.query_map(params![cutoff, MAX_RECORDS as i64], |row| {
        Ok(FileRecord {
            filename: row.get(0)?,
            path: row.get(1)?,
            format: row.get(2)?,
            ok: row.get::<_, i64>(3)? != 0,
            chars: row.get::<_, i64>(4)? as u64,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}
