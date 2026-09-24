//! The history store: one SQLite database (`$XDG_DATA_HOME/dettivo/dettivo.db`,
//! WAL, foreign keys on, a five second busy timeout) that keeps every
//! dictation item with the macOS `DictationItem` fields, an FTS5 index over
//! their text kept in step by triggers, numbered migrations that back the
//! file up before they run, retention over audio and rows, the twelve-item
//! QA seed and the export renderers, plus the `meetings` table (migration
//! `0004-meetings`, ADR 0027; speakers in `0006-speakers`, ADR 0035; notes
//! and analysis in `0007-notes-analysis`, ADR 0036; the `queued` analysis
//! status in `0008-analysis-queued`, ADR 0061) and the one timeline over
//! both kinds.

pub mod combined_search;
pub mod dictations;
pub mod export;
pub mod item;
pub mod meeting_artifacts;
pub mod meeting_export;
pub mod meeting_export_md;
pub mod meeting_notes;
pub mod meeting_reads;
pub mod meeting_row;
pub mod meetings;
pub mod migrate;
pub mod retention;
pub mod search;
pub mod seed;
pub mod seed_meetings;
pub mod speakers;
pub mod time;
pub mod timeline;
pub mod zip;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Package names of the workspace crates this crate builds on.
pub const UPSTREAM: &[&str] = &[dettivo_proto::CRATE_NAME];

/// How long a statement waits for another writer before it gives up.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Why a store call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// Another process held the database past the busy timeout.
    Busy,
    /// A migration failed; the backup written before it is named.
    Migration {
        /// The migration's name (`0001-dictations`).
        name: String,
        /// The backup file, when one was written.
        backup: Option<PathBuf>,
        /// SQLite's message.
        message: String,
    },
    /// No item has this id.
    NotFound(String),
    /// A search query the store refuses; the message names the limit.
    InvalidQuery(String),
    /// Any other SQLite failure.
    Sqlite(String),
    /// A file operation failed.
    Io(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => write!(f, "the database is busy"),
            Self::Migration {
                name,
                backup,
                message,
            } => match backup {
                Some(b) => write!(
                    f,
                    "migration {name} failed: {message}; the backup is {}",
                    b.display()
                ),
                None => write!(f, "migration {name} failed: {message}"),
            },
            Self::NotFound(id) => write!(f, "no item {id}"),
            Self::InvalidQuery(m) => f.write_str(m),
            Self::Sqlite(m) => write!(f, "database: {m}"),
            Self::Io(m) => write!(f, "file: {m}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        match &e {
            rusqlite::Error::SqliteFailure(inner, _)
                if matches!(
                    inner.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) =>
            {
                Self::Busy
            }
            _ => Self::Sqlite(e.to_string()),
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// An open database.
pub struct Store {
    conn: Connection,
    path: PathBuf,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").field("path", &self.path).finish()
    }
}

impl Store {
    /// Opens (creating when absent) the database at `path`, sets WAL,
    /// foreign keys and the busy timeout, and runs the pending migrations.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::open_with(path, migrate::MIGRATIONS, BUSY_TIMEOUT)
    }

    /// `open` with an explicit migration list and busy timeout (tests).
    pub fn open_with(
        path: &Path,
        migrations: &[migrate::Migration],
        busy_timeout: Duration,
    ) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(busy_timeout)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        migrate::apply(&mut conn, path, migrations)?;
        Ok(Self {
            conn,
            path: path.to_path_buf(),
        })
    }

    /// An in-memory store (tests).
    pub fn in_memory() -> Result<Self, StoreError> {
        let mut conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate::apply(&mut conn, Path::new(":memory:"), migrate::MIGRATIONS)?;
        Ok(Self {
            conn,
            path: PathBuf::from(":memory:"),
        })
    }

    /// The database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The connection, for the repositories in this crate.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// The schema version in force.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        migrate::current_version(&self.conn)
    }

    /// The name of the last applied migration.
    pub fn last_migration(&self) -> Result<Option<String>, StoreError> {
        Ok(migrate::last_applied(&self.conn)?.map(|(_, name, _)| name))
    }

    /// The size of the database file plus its WAL, in bytes.
    pub fn size_bytes(&self) -> u64 {
        let size = |p: PathBuf| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let mut wal = self.path.as_os_str().to_owned();
        wal.push("-wal");
        size(self.path.clone()) + size(PathBuf::from(wal))
    }

    /// The journal mode and foreign-key setting in force, for tests.
    pub fn pragmas(&self) -> Result<(String, bool), StoreError> {
        let journal: String = self
            .conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        let fk: i64 = self
            .conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))?;
        Ok((journal, fk == 1))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-storage");
    }

    #[test]
    fn upstream_edges_resolve() {
        assert_eq!(super::UPSTREAM, &["dettivo-proto"]);
    }
}
