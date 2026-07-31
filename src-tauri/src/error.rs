use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database operation failed: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("database migration failed: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("failed to create database directory {path}: {source}")]
    CreateDatabaseDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("database lock was poisoned")]
    DatabaseLockPoisoned,

    #[error("invalid activity session: {0}")]
    InvalidSession(String),

    #[error("invalid idle threshold: {0}")]
    InvalidIdleThreshold(String),

    #[error("platform activity query failed: {0}")]
    Platform(String),

    #[error("system clock is before the Unix epoch")]
    InvalidSystemClock,

    #[error("invalid monitor configuration: {0}")]
    InvalidMonitorConfiguration(String),

    #[error("invalid time range: {0}")]
    InvalidTimeRange(String),

    #[error("activity session {0} was not found")]
    SessionNotFound(i64),
}

pub type AppResult<T> = Result<T, AppError>;
