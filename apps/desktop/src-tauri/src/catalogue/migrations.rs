use rusqlite::{Connection, Transaction};
use crate::error::AppError;

pub struct Migration {
    pub version: i32,
    pub description: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "001_initial_catalogue",
        sql: r#"
        CREATE TABLE IF NOT EXISTS shows (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS episodes (
            id TEXT PRIMARY KEY,
            show_id TEXT NOT NULL,
            source_path TEXT NOT NULL,
            filename TEXT NOT NULL,
            file_size_bytes INTEGER NOT NULL,
            duration_seconds REAL NOT NULL,
            format TEXT NOT NULL,
            codec TEXT NOT NULL,
            sample_rate INTEGER NOT NULL,
            channels INTEGER NOT NULL,
            bitrate INTEGER,
            integrated_loudness_lufs REAL,
            true_peak_dbtp REAL,
            leading_silence_seconds REAL NOT NULL,
            trailing_silence_seconds REAL NOT NULL,
            clipping_evidence TEXT NOT NULL,
            overall_assessment_status TEXT NOT NULL,
            assessment_profile_id TEXT NOT NULL,
            assessment_profile_version TEXT NOT NULL,
            analysed_at TEXT NOT NULL,
            source_modified_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            assessment_json TEXT,
            FOREIGN KEY (show_id) REFERENCES shows(id) ON DELETE CASCADE,
            UNIQUE(show_id, source_path)
        );

        CREATE INDEX IF NOT EXISTS idx_episodes_show_id ON episodes(show_id);
        CREATE INDEX IF NOT EXISTS idx_episodes_show_source ON episodes(show_id, source_path);
        "#,
    },
    Migration {
        version: 2,
        description: "002_normalize_overall_assessment_status",
        sql: r#"
        UPDATE episodes
        SET overall_assessment_status = 'NEEDS_ATTENTION'
        WHERE overall_assessment_status = 'NEEDSATTENTION';
        "#,
    },
];

pub fn run_migrations(conn: &mut Connection) -> Result<(), AppError> {
    // Enforce foreign keys
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| AppError::DatabaseError(format!("Failed to enable foreign keys: {}", e)))?;

    // Create schema_migrations table if not exists
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
        [],
    )
    .map_err(|e| AppError::DatabaseError(format!("Failed to create schema_migrations table: {}", e)))?;

    // Get current applied migration versions
    let applied_versions: Vec<i32> = {
        let mut stmt = conn
            .prepare("SELECT version FROM schema_migrations ORDER BY version ASC")
            .map_err(|e| AppError::DatabaseError(format!("Failed to prepare migration query: {}", e)))?;

        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| AppError::DatabaseError(format!("Failed to query applied migrations: {}", e)))?;

        let mut list = Vec::new();
        for r in rows {
            if let Ok(v) = r {
                list.push(v);
            }
        }
        list
    };

    for migration in MIGRATIONS {
        if !applied_versions.contains(&migration.version) {
            log::info!(
                "Applying catalogue migration {} ({})",
                migration.version,
                migration.description
            );

            let tx = conn
                .transaction()
                .map_err(|e| AppError::DatabaseError(format!("Failed to start migration transaction: {}", e)))?;

            apply_migration(&tx, migration)?;

            tx.commit()
                .map_err(|e| AppError::DatabaseError(format!("Failed to commit migration transaction: {}", e)))?;
        }
    }


    Ok(())
}

fn apply_migration(tx: &Transaction, migration: &Migration) -> Result<(), AppError> {
    tx.execute_batch(migration.sql).map_err(|e| {
        AppError::DatabaseError(format!(
            "Failed executing migration {}: {}",
            migration.version, e
        ))
    })?;

    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        rusqlite::params![migration.version, now],
    )
    .map_err(|e| {
        AppError::DatabaseError(format!(
            "Failed recording migration {}: {}",
            migration.version, e
        ))
    })?;

    Ok(())
}
