//! The files the antivirus already found clean, so a quick scan (and a custom one that asks for it)
//! sends to ClamAV only what changed since.
//!
//! A file is the same one when its device, inode, size, time of modification and time of change
//! (ctime) are: the mtime can be set back by whoever writes the file, the ctime cannot. A result
//! holds only for the options it was found with (archives, PUA, largest file) and for 30 days: then
//! the file goes to ClamAV again, with the signatures of the day. A file found infected is never
//! recorded. The archive is a SQLite database in the state directory of the antivirus, and a
//! failure of it never fails a scan: the scan is then whole.

use super::types::ScanOptions;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    QueryBuilder, Row, Sqlite, SqlitePool,
};
use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::Metadata,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::Path,
    time::Duration,
};

/// After this long a clean result no longer holds, and the file is scanned again.
pub const RESCAN_AFTER_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Files looked up or recorded in one statement.
pub const BATCH: usize = 400;

/// What makes a file the same one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub device: u64,
    pub inode: u64,
    pub size: u64,
    pub modified_ns: i128,
    pub changed_ns: i128,
}

impl Stamp {
    pub fn of(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified_ns: i128::from(metadata.mtime()) * 1_000_000_000
                + i128::from(metadata.mtime_nsec()),
            changed_ns: i128::from(metadata.ctime()) * 1_000_000_000
                + i128::from(metadata.ctime_nsec()),
        }
    }
}

/// The options a clean result depends on.
pub fn fingerprint(options: &ScanOptions) -> String {
    format!(
        "archives={};pua={};max={}",
        options.archives, options.detect_pua, options.max_file_mb
    )
}

fn integer(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn nanoseconds(value: i128) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub struct ScanCache {
    pool: SqlitePool,
}

impl ScanCache {
    pub async fn open(path: &Path) -> Result<Self, String> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|error| error.to_string())?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS clean_files (
                path        BLOB    PRIMARY KEY,
                device      INTEGER NOT NULL,
                inode       INTEGER NOT NULL,
                size        INTEGER NOT NULL,
                modified_ns INTEGER NOT NULL,
                changed_ns  INTEGER NOT NULL,
                options     TEXT    NOT NULL,
                scanned_at  INTEGER NOT NULL
            ) STRICT, WITHOUT ROWID",
        )
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;
        Ok(Self { pool })
    }

    /// Of the files given, those found clean with the same stamp and options in the last 30 days.
    pub async fn unchanged(
        &self,
        files: &[(Vec<u8>, Stamp)],
        options: &str,
        now_ms: i64,
    ) -> Result<HashSet<Vec<u8>>, String> {
        if files.is_empty() {
            return Ok(HashSet::new());
        }
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT path, device, inode, size, modified_ns, changed_ns FROM clean_files \
             WHERE options = ",
        );
        query
            .push_bind(options)
            .push(" AND scanned_at > ")
            .push_bind(now_ms - RESCAN_AFTER_MS)
            .push(" AND path IN (");
        let mut separated = query.separated(", ");
        for (path, _) in files {
            separated.push_bind(path.as_slice());
        }
        separated.push_unseparated(")");
        let rows = query
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| error.to_string())?;
        let known: Vec<(Vec<u8>, [i64; 5])> = rows
            .iter()
            .map(|row| {
                (
                    row.get::<Vec<u8>, _>(0),
                    [row.get(1), row.get(2), row.get(3), row.get(4), row.get(5)],
                )
            })
            .collect();
        Ok(files
            .iter()
            .filter(|(path, stamp)| {
                known.iter().any(|(known_path, values)| {
                    known_path == path
                        && *values
                            == [
                                integer(stamp.device),
                                integer(stamp.inode),
                                integer(stamp.size),
                                nanoseconds(stamp.modified_ns),
                                nanoseconds(stamp.changed_ns),
                            ]
                })
            })
            .map(|(path, _)| path.clone())
            .collect())
    }

    /// Records files ClamAV found clean.
    pub async fn record(
        &self,
        files: &[(Vec<u8>, Stamp)],
        options: &str,
        now_ms: i64,
    ) -> Result<(), String> {
        if files.is_empty() {
            return Ok(());
        }
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT OR REPLACE INTO clean_files \
             (path, device, inode, size, modified_ns, changed_ns, options, scanned_at) ",
        );
        query.push_values(files, |mut row, (path, stamp)| {
            row.push_bind(path.as_slice())
                .push_bind(integer(stamp.device))
                .push_bind(integer(stamp.inode))
                .push_bind(integer(stamp.size))
                .push_bind(nanoseconds(stamp.modified_ns))
                .push_bind(nanoseconds(stamp.changed_ns))
                .push_bind(options)
                .push_bind(now_ms);
        });
        query
            .build()
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// A file found infected is no longer clean.
    pub async fn forget(&self, path: &OsStr) -> Result<(), String> {
        sqlx::query("DELETE FROM clean_files WHERE path = ?")
            .bind(path.as_bytes())
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Results too old to be used: those of files gone since go with them.
    pub async fn prune(&self, now_ms: i64) -> Result<(), String> {
        sqlx::query("DELETE FROM clean_files WHERE scanned_at <= ?")
            .bind(now_ms - RESCAN_AFTER_MS)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// How many files the archive holds.
    pub async fn count(&self) -> Result<u64, String> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM clean_files")
            .fetch_one(&self.pool)
            .await
            .map(|count| u64::try_from(count).unwrap_or_default())
            .map_err(|error| error.to_string())
    }

    pub async fn close(self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
#[path = "../../../../../tests/apps/services/antivirus/scan_cache.rs"]
mod tests;
