//! SQLite database of the launcher

use sqlx::{
    migrate::{MigrateError, Migrator},
    sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
    },
};
use std::{
    env, fmt, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

/// Migrations of `backend/axum/migrations`, embedded in the binary and applied at startup.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const FILE_NAME: &str = "launcher.db";
/// Where the copy made with the launcher running is kept, beside the database.
const COPY_NAME: &str = "copies/launcher.db";
const COPY_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

const FILE_MODE: u32 = 0o600;
const DIRECTORY_MODE: u32 = 0o700;

#[derive(Debug)]
pub enum DatabaseError {
    Location,
    Io(io::Error),
    Open(sqlx::Error),
    Migrate(MigrateError),
}
impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Location => write!(
                f,
                "no database location: set NAS_DB_PATH or run under systemd with StateDirectory="
            ),
            Self::Io(error) => write!(f, "cannot prepare the database file: {error}"),
            Self::Open(error) => write!(f, "cannot open the database: {error}"),
            Self::Migrate(error) => write!(f, "cannot apply the migrations: {error}"),
        }
    }
}
impl std::error::Error for DatabaseError {}

pub fn location() -> Result<PathBuf, DatabaseError> {
    if let Some(path) = env::var("NAS_DB_PATH").ok().filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if let Some(dir) = env::var("STATE_DIRECTORY")
        .ok()
        .and_then(|dirs| dirs.split(':').next().map(str::to_owned))
        .filter(|dir| !dir.is_empty())
    {
        return Ok(PathBuf::from(dir).join(FILE_NAME));
    }
    if cfg!(debug_assertions) {
        return Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../data")
            .join(FILE_NAME));
    }
    Err(DatabaseError::Location)
}

pub fn beside(name: &str) -> Option<PathBuf> {
    location()
        .ok()?
        .parent()
        .map(|directory| directory.join(name))
}

pub async fn open(path: &Path) -> Result<SqlitePool, DatabaseError> {
    prepare_file(path).await.map_err(DatabaseError::Io)?;
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .map_err(DatabaseError::Open)?;
    migrate(&pool).await?;
    Ok(pool)
}

pub async fn open_in_memory() -> Result<SqlitePool, DatabaseError> {
    let options = "sqlite::memory:"
        .parse::<SqliteConnectOptions>()
        .map_err(DatabaseError::Open)?
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await
        .map_err(DatabaseError::Open)?;
    migrate(&pool).await?;
    Ok(pool)
}

/// A consistent copy of the database at `target`, made while the launcher keeps working: `VACUUM
/// INTO` reads one snapshot of the database and its write-ahead log, which copying the files would
/// not. Written aside and renamed, so `target` is never half a copy, and readable by the launcher
/// only, like the database.
pub async fn copy(pool: &SqlitePool, target: &Path) -> Result<(), DatabaseError> {
    let mut aside = target.as_os_str().to_owned();
    aside.push(".new");
    let aside = PathBuf::from(aside);
    if let Some(dir) = target.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        tokio::fs::DirBuilder::new()
            .recursive(true)
            .mode(DIRECTORY_MODE)
            .create(dir)
            .await
            .map_err(DatabaseError::Io)?;
    }
    match tokio::fs::remove_file(&aside).await {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(DatabaseError::Io(error))
        }
        _ => {}
    }
    sqlx::query("VACUUM INTO ?")
        .bind(aside.to_string_lossy().into_owned())
        .execute(pool)
        .await
        .map_err(DatabaseError::Open)?;
    tokio::fs::set_permissions(&aside, std::fs::Permissions::from_mode(FILE_MODE))
        .await
        .map_err(DatabaseError::Io)?;
    tokio::fs::rename(&aside, target)
        .await
        .map_err(DatabaseError::Io)
}

/// The copy beside the database at `path`, made at start and once a day: what a backup takes
/// instead of the live files.
pub async fn keep_copy(pool: SqlitePool, path: PathBuf) {
    let Some(target) = path.parent().map(|dir| dir.join(COPY_NAME)) else {
        return;
    };
    let mut every = tokio::time::interval(COPY_EVERY);
    loop {
        every.tick().await;
        if let Err(error) = copy(&pool, &target).await {
            eprintln!("the copy of the database was not made: {error}");
        }
    }
}

async fn migrate(pool: &SqlitePool) -> Result<(), DatabaseError> {
    MIGRATOR.run(pool).await.map_err(DatabaseError::Migrate)
}

async fn prepare_file(path: &Path) -> io::Result<()> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        tokio::fs::DirBuilder::new()
            .recursive(true)
            .mode(DIRECTORY_MODE)
            .create(dir)
            .await?;
    }
    tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(FILE_MODE)
        .open(path)
        .await?;

    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.as_os_str().to_owned();
        file.push(suffix);
        match tokio::fs::set_permissions(&file, std::fs::Permissions::from_mode(FILE_MODE)).await {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/services/database.rs"]
mod tests;
