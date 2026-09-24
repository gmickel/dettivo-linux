//! Numbered migrations. Each runs once, in a transaction, recorded in
//! `schema_migrations`; before the first pending one runs against a
//! database that already has content, a consistent copy is written next
//! to it as `<db>.bak-<version>` so a failure names both the migration and
//! the file to restore.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::StoreError;
use crate::time::now_iso;

/// One migration: a version, a name and the SQL it runs.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// The version this migration brings the schema to.
    pub version: u32,
    /// The name, as recorded in `schema_migrations`.
    pub name: &'static str,
    /// The statements, run as one batch inside a transaction.
    pub sql: &'static str,
}

/// Every migration, in order.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001-dictations",
        sql: include_str!("../migrations/0001-dictations.sql"),
    },
    Migration {
        version: 2,
        name: "0002-segments",
        sql: include_str!("../migrations/0002-segments.sql"),
    },
    Migration {
        version: 3,
        name: "0003-polish-policy",
        sql: include_str!("../migrations/0003-polish-policy.sql"),
    },
    Migration {
        version: 4,
        name: "0004-meetings",
        sql: include_str!("../migrations/0004-meetings.sql"),
    },
    Migration {
        version: 5,
        name: "0005-timings",
        sql: include_str!("../migrations/0005-timings.sql"),
    },
    Migration {
        version: 6,
        name: "0006-speakers",
        sql: include_str!("../migrations/0006-speakers.sql"),
    },
    Migration {
        version: 7,
        name: "0007-notes-analysis",
        sql: include_str!("../migrations/0007-notes-analysis.sql"),
    },
    Migration {
        version: 8,
        name: "0008-analysis-queued",
        sql: include_str!("../migrations/0008-analysis-queued.sql"),
    },
];

/// The version the current code expects.
pub const CURRENT_VERSION: u32 = 8;

const TABLE: &str = "CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
)";

/// The highest applied version, 0 for an empty database.
pub fn current_version(conn: &Connection) -> Result<u32, StoreError> {
    conn.execute_batch(TABLE)?;
    let version: Option<u32> =
        conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
            r.get(0)
        })?;
    Ok(version.unwrap_or(0))
}

/// The last applied migration: version, name and time.
pub fn last_applied(conn: &Connection) -> Result<Option<(u32, String, String)>, StoreError> {
    conn.execute_batch(TABLE)?;
    Ok(conn
        .query_row(
            "SELECT version, name, applied_at FROM schema_migrations ORDER BY version DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
}

/// Applies every migration above the current version.
pub fn apply(
    conn: &mut Connection,
    path: &Path,
    migrations: &[Migration],
) -> Result<(), StoreError> {
    let current = current_version(conn)?;
    let pending: Vec<&Migration> = migrations.iter().filter(|m| m.version > current).collect();
    if pending.is_empty() {
        return Ok(());
    }
    let backup = if current > 0 && path.is_file() {
        Some(backup(conn, path, current)?)
    } else {
        None
    };
    for m in pending {
        let tx = conn.transaction()?;
        let run = tx.execute_batch(m.sql).and_then(|()| {
            tx.execute(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![m.version, m.name, now_iso()],
            )
        });
        if let Err(e) = run {
            drop(tx);
            return Err(StoreError::Migration {
                name: m.name.to_string(),
                backup,
                message: e.to_string(),
            });
        }
        tx.commit()?;
        tracing::info!(migration = m.name, "history schema migrated");
    }
    Ok(())
}

/// A consistent copy of the database (`VACUUM INTO`), WAL contents included.
fn backup(conn: &Connection, path: &Path, version: u32) -> Result<PathBuf, StoreError> {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".bak-{version}"));
    let target = PathBuf::from(name);
    if target.exists() {
        std::fs::remove_file(&target)?;
    }
    conn.execute(
        "VACUUM INTO ?1",
        params![target.to_string_lossy().into_owned()],
    )?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_numbered_from_one_without_gaps() {
        for (i, m) in MIGRATIONS.iter().enumerate() {
            assert_eq!(m.version, i as u32 + 1, "{}", m.name);
            assert!(m.name.starts_with(&format!("{:04}-", m.version)));
        }
        assert_eq!(MIGRATIONS.last().unwrap().version, CURRENT_VERSION);
    }
}
