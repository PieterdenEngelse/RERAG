use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CorporaError {
    #[error("invalid slug '{0}': must be 1-64 lowercase alphanumeric/hyphen chars, start and end with alphanumeric")]
    InvalidSlug(String),
    #[error("corpus not found: '{0}'")]
    NotFound(String),
    #[error("cannot delete the default corpus")]
    CannotDeleteDefault,
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, CorporaError>;

#[derive(Debug, Clone, Serialize)]
pub struct Corpus {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub created_at: String,
}

/// Per-corpus settings stored as JSON in `corpora.settings`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorpusSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_top_k: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunker_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_metric: Option<crate::config::DistanceMetric>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hnsw_ef_construction: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hnsw_ef_search: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pq_subvectors: Option<usize>,
}

/// Build-time parameters recorded after each reindex. Used to detect settings drift.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorpusBuildMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunker_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_metric: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hnsw_ef_construction: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hnsw_ef_search: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pq_subvectors: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub built_at: Option<String>,
}

/// Settings for the agent memory vector store (separate from per-corpus settings).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentMemorySettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_metric: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,
}

pub fn get_corpus_settings(conn: &Connection, slug: &str) -> Result<CorpusSettings> {
    let json_opt: Option<String> = conn
        .query_row(
            "SELECT settings FROM corpora WHERE slug = ?1",
            params![slug],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    Ok(json_opt
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

pub fn set_corpus_settings(conn: &Connection, slug: &str, settings: &CorpusSettings) -> Result<()> {
    let json = serde_json::to_string(settings)
        .map_err(|e| CorporaError::Db(rusqlite::Error::InvalidParameterName(e.to_string())))?;
    let updated = conn.execute(
        "UPDATE corpora SET settings = ?1 WHERE slug = ?2",
        params![json, slug],
    )?;
    if updated == 0 {
        return Err(CorporaError::NotFound(slug.to_string()));
    }
    Ok(())
}

pub fn get_corpus_build_meta(conn: &Connection, slug: &str) -> Result<CorpusBuildMeta> {
    let json_opt: Option<String> = conn
        .query_row(
            "SELECT build_meta FROM corpora WHERE slug = ?1",
            params![slug],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    Ok(json_opt
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

pub fn set_corpus_build_meta(conn: &Connection, slug: &str, meta: &CorpusBuildMeta) -> Result<()> {
    let json = serde_json::to_string(meta)
        .map_err(|e| CorporaError::Db(rusqlite::Error::InvalidParameterName(e.to_string())))?;
    let updated = conn.execute(
        "UPDATE corpora SET build_meta = ?1 WHERE slug = ?2",
        params![json, slug],
    )?;
    if updated == 0 {
        return Err(CorporaError::NotFound(slug.to_string()));
    }
    Ok(())
}

pub fn get_agent_memory_settings(conn: &Connection) -> AgentMemorySettings {
    conn.query_row(
        "SELECT settings_json FROM agent_memory_settings WHERE id = 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default()
}

pub fn set_agent_memory_settings(
    conn: &Connection,
    settings: &AgentMemorySettings,
) -> std::result::Result<(), String> {
    let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO agent_memory_settings (id, settings_json) VALUES (1, ?1)
         ON CONFLICT(id) DO UPDATE SET settings_json = ?1",
        params![json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Returns true if `slug` is a valid corpus identifier:
/// lowercase alphanumeric + hyphens, 1–64 chars, first and last char alphanumeric.
pub fn validate_slug(slug: &str) -> bool {
    if slug.is_empty() || slug.len() > 64 {
        return false;
    }
    let bytes = slug.as_bytes();
    let first = bytes[0];
    let last = bytes[bytes.len() - 1];
    if !first.is_ascii_alphanumeric() || !last.is_ascii_alphanumeric() {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub fn create_corpus(conn: &Connection, slug: &str, name: &str) -> Result<Corpus> {
    if !validate_slug(slug) {
        return Err(CorporaError::InvalidSlug(slug.to_string()));
    }
    let id: String = conn.query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))?;
    conn.execute(
        "INSERT INTO corpora (id, slug, name) VALUES (?1, ?2, ?3)",
        params![id, slug, name],
    )?;
    get_corpus_by_slug(conn, slug)?.ok_or_else(|| CorporaError::NotFound(slug.to_string()))
}

pub fn list_corpora(conn: &Connection) -> Result<Vec<Corpus>> {
    let mut stmt =
        conn.prepare("SELECT id, slug, name, created_at FROM corpora ORDER BY created_at ASC")?;
    let corpora = stmt
        .query_map([], |row| {
            Ok(Corpus {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(corpora)
}

pub fn get_corpus_by_slug(conn: &Connection, slug: &str) -> Result<Option<Corpus>> {
    match conn.query_row(
        "SELECT id, slug, name, created_at FROM corpora WHERE slug = ?1",
        params![slug],
        |row| {
            Ok(Corpus {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                created_at: row.get(3)?,
            })
        },
    ) {
        Ok(c) => Ok(Some(c)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(CorporaError::Db(e)),
    }
}

pub fn rename_corpus(conn: &Connection, slug: &str, new_name: &str) -> Result<()> {
    let updated = conn.execute(
        "UPDATE corpora SET name = ?1 WHERE slug = ?2",
        params![new_name, slug],
    )?;
    if updated == 0 {
        return Err(CorporaError::NotFound(slug.to_string()));
    }
    Ok(())
}

/// Delete a corpus and all its documents/chunks/embeddings.
/// The 'default' corpus cannot be deleted.
pub fn delete_corpus(conn: &Connection, slug: &str) -> Result<()> {
    if slug == "default" {
        return Err(CorporaError::CannotDeleteDefault);
    }
    let corpus =
        get_corpus_by_slug(conn, slug)?.ok_or_else(|| CorporaError::NotFound(slug.to_string()))?;
    // Foreign-key cascades handle chunks and embeddings when documents are deleted,
    // but corpus_id is nullable, so delete orphan rows explicitly.
    conn.execute(
        "DELETE FROM embeddings WHERE chunk_id IN (
             SELECT c.id FROM chunks c
             JOIN documents d ON c.document_id = d.id
             WHERE d.corpus_id = ?1
         )",
        params![corpus.id],
    )?;
    conn.execute(
        "DELETE FROM chunks WHERE document_id IN (
             SELECT id FROM documents WHERE corpus_id = ?1
         )",
        params![corpus.id],
    )?;
    conn.execute(
        "DELETE FROM documents WHERE corpus_id = ?1",
        params![corpus.id],
    )?;
    conn.execute("DELETE FROM corpora WHERE id = ?1", params![corpus.id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/schema.sql"))
            .unwrap();
        // run v14 migration inline
        conn.execute_batch(
            "INSERT OR IGNORE INTO corpora (id, slug, name)
             VALUES (lower(hex(randomblob(16))), 'default', 'Default')",
        )
        .unwrap();
        conn
    }

    #[test]
    fn validate_slug_ok() {
        assert!(validate_slug("philosophy"));
        assert!(validate_slug("legal-2024"));
        assert!(validate_slug("a"));
        assert!(validate_slug("a1"));
    }

    #[test]
    fn validate_slug_reject() {
        assert!(!validate_slug(""));
        assert!(!validate_slug("-starts-with-hyphen"));
        assert!(!validate_slug("ends-with-hyphen-"));
        assert!(!validate_slug("UPPERCASE"));
        assert!(!validate_slug(&"a".repeat(65)));
    }

    #[test]
    fn create_and_list() {
        let conn = fresh_db();
        let c = create_corpus(&conn, "philosophy", "Philosophy").unwrap();
        assert_eq!(c.slug, "philosophy");
        let list = list_corpora(&conn).unwrap();
        assert_eq!(list.len(), 2); // default + philosophy
    }

    #[test]
    fn cannot_delete_default() {
        let conn = fresh_db();
        assert!(matches!(
            delete_corpus(&conn, "default"),
            Err(CorporaError::CannotDeleteDefault)
        ));
    }
}
