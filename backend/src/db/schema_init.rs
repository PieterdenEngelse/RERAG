// ag/src/db/schema_init.rs v17.0.0
use crate::path_manager::PathManager;
use crate::perf::sqlite_opt::{optimize_connection, SqliteConfig};
use rusqlite::{params, Connection, Result as SqlResult};
use tracing::info;

pub struct SchemaInitializer;

impl SchemaInitializer {
    pub fn init(db_conn: &Connection) -> SqlResult<()> {
        info!("Initializing database schema v17.0.0");

        // Apply SQLite performance optimizations (WAL mode, mmap, etc.)
        let config = SqliteConfig::default();
        if let Err(e) = optimize_connection(db_conn, &config) {
            tracing::warn!("Failed to apply SQLite optimizations: {}", e);
        }

        let schema_sql = include_str!("../db/schema.sql");
        db_conn.execute_batch(schema_sql)?;
        Self::run_v14_migration(db_conn)?;
        Self::run_v15_migration(db_conn)?;
        Self::run_v16_migration(db_conn)?;
        Self::run_v17_migration(db_conn)?;
        Self::run_v18_migration(db_conn)?;
        info!("Database schema initialized with WAL mode");
        Ok(())
    }

    /// Add corpus_id / corpus_slug columns to existing tables (idempotent).
    /// Then ensure the 'default' corpus row exists and backfill NULL corpus_id rows.
    fn run_v14_migration(conn: &Connection) -> SqlResult<()> {
        // --- Phase 1: column additions ---
        for (table, column, ddl) in &[
            ("documents",  "corpus_id",   "ALTER TABLE documents  ADD COLUMN corpus_id TEXT REFERENCES corpora(id)"),
            ("chunks",     "corpus_id",   "ALTER TABLE chunks     ADD COLUMN corpus_id TEXT REFERENCES corpora(id)"),
            ("embeddings", "corpus_id",   "ALTER TABLE embeddings ADD COLUMN corpus_id TEXT REFERENCES corpora(id)"),
            ("golden_sample",      "corpus_slug", "ALTER TABLE golden_sample      ADD COLUMN corpus_slug TEXT NOT NULL DEFAULT 'default'"),
            ("golden_sample_meta", "corpus_slug", "ALTER TABLE golden_sample_meta ADD COLUMN corpus_slug TEXT NOT NULL DEFAULT 'default'"),
        ] {
            if !Self::column_exists(conn, table, column)? {
                conn.execute_batch(ddl)?;
                info!("corpus migration: added {}.{}", table, column);
            }
        }

        // --- Phase 7: ensure default corpus + backfill ---
        let default_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM corpora WHERE slug = 'default'",
            [],
            |row| row.get(0),
        )?;
        if default_count == 0 {
            let id: String =
                conn.query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))?;
            conn.execute(
                "INSERT INTO corpora (id, slug, name) VALUES (?1, 'default', 'Default')",
                params![id],
            )?;
            info!("corpus migration: created default corpus id={}", id);
        }

        let default_id: String =
            conn.query_row("SELECT id FROM corpora WHERE slug = 'default'", [], |row| {
                row.get(0)
            })?;

        for table in &["documents", "chunks", "embeddings"] {
            let updated = conn.execute(
                &format!(
                    "UPDATE {} SET corpus_id = ?1 WHERE corpus_id IS NULL",
                    table
                ),
                params![default_id],
            )?;
            if updated > 0 {
                info!("corpus migration: backfilled {} rows in {}", updated, table);
            }
        }

        Ok(())
    }

    /// Add `settings` TEXT column to `corpora` (idempotent).
    fn run_v15_migration(conn: &Connection) -> SqlResult<()> {
        if !Self::column_exists(conn, "corpora", "settings")? {
            conn.execute_batch("ALTER TABLE corpora ADD COLUMN settings TEXT")?;
            info!("corpus migration v15: added corpora.settings column");
        }
        Ok(())
    }

    /// Add `build_meta` TEXT column to `corpora`; create `agent_memory_settings` table.
    fn run_v16_migration(conn: &Connection) -> SqlResult<()> {
        if !Self::column_exists(conn, "corpora", "build_meta")? {
            conn.execute_batch("ALTER TABLE corpora ADD COLUMN build_meta TEXT")?;
            info!("corpus migration v16: added corpora.build_meta column");
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_memory_settings (
                id            INTEGER PRIMARY KEY CHECK (id = 1),
                settings_json TEXT NOT NULL DEFAULT '{}'
            );
            INSERT OR IGNORE INTO agent_memory_settings (id, settings_json) VALUES (1, '{}');",
        )?;
        info!("corpus migration v16: ensured agent_memory_settings table");
        Ok(())
    }

    /// Add `description` TEXT column to `corpora`.
    fn run_v17_migration(conn: &Connection) -> SqlResult<()> {
        if !Self::column_exists(conn, "corpora", "description")? {
            conn.execute_batch(
                "ALTER TABLE corpora ADD COLUMN description TEXT NOT NULL DEFAULT ''",
            )?;
            info!("corpus migration v17: added corpora.description column");
        }
        Ok(())
    }

    /// Add `watch_dir` TEXT column to `corpora` — per-corpus override for the
    /// directory the file watcher monitors. NULL means "fall back to the
    /// PathManager-derived default".
    fn run_v18_migration(conn: &Connection) -> SqlResult<()> {
        if !Self::column_exists(conn, "corpora", "watch_dir")? {
            conn.execute_batch("ALTER TABLE corpora ADD COLUMN watch_dir TEXT")?;
            info!("corpus migration v18: added corpora.watch_dir column");
        }
        Ok(())
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> SqlResult<bool> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let names: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(names.iter().any(|n| n == column))
    }

    pub fn create_fresh_db(path_manager: &PathManager) -> SqlResult<Connection> {
        let db_path = path_manager.db_path("documents");
        info!("Creating database at: {}", db_path.display());
        let conn = Connection::open(&db_path)?;
        Self::init(&conn)?;
        Ok(conn)
    }

    pub fn migrate(db_conn: &Connection, target_version: &str) -> SqlResult<()> {
        info!("Migrating database to version {}", target_version);
        // init() already applies all migrations idempotently
        Self::init(db_conn)?;
        Ok(())
    }
}
