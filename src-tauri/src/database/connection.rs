use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use rusqlite::{Connection, OpenFlags};

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AppError::CreateDatabaseDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::configure(connection, true)
    }

    #[cfg(test)]
    pub(crate) fn in_memory() -> AppResult<Self> {
        Self::configure(Connection::open_in_memory()?, false)
    }

    fn configure(mut connection: Connection, persistent: bool) -> AppResult<Self> {
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(Duration::from_secs(5))?;

        if persistent {
            connection.pragma_update(None, "journal_mode", "WAL")?;
            connection.pragma_update(None, "synchronous", "NORMAL")?;
            // Checkpoint after roughly 1 MiB of 4 KiB WAL pages instead of
            // SQLite's default ~4 MiB threshold. The journal size limit keeps
            // the reusable WAL allocation bounded after a checkpoint.
            connection.pragma_update(None, "wal_autocheckpoint", 256)?;
            connection.pragma_update(None, "journal_size_limit", 1_048_576)?;
        }

        super::migration::migrations().to_latest(&mut connection)?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub(crate) fn lock(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| AppError::DatabaseLockPoisoned)
    }
}

#[cfg(test)]
mod tests {
    use rusqlite_migration::SchemaVersion;

    use super::Database;
    use crate::database::migration::migrations;

    #[test]
    fn applies_all_migrations() {
        let database = Database::in_memory().expect("database should open");
        let connection = database.lock().expect("database should lock");

        assert_eq!(
            migrations()
                .current_version(&connection)
                .expect("schema version should be readable"),
            SchemaVersion::Inside(std::num::NonZeroUsize::new(7).expect("seven is non-zero"))
        );
        assert_eq!(
            connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .expect("foreign key pragma should be readable"),
            1
        );
    }

    #[test]
    fn persistent_database_can_be_reopened_without_reapplying_schema() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("nested").join("watchhouse.sqlite3");

        drop(Database::open(&path).expect("database should open"));
        let reopened = Database::open(&path).expect("database should reopen");
        let connection = reopened.lock().expect("database should lock");

        assert_eq!(
            migrations()
                .pending_migrations(&connection)
                .expect("migration state should be readable"),
            0
        );
        assert_eq!(
            connection
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .expect("journal mode should be readable")
                .to_ascii_lowercase(),
            "wal"
        );
        assert_eq!(
            connection
                .query_row("PRAGMA wal_autocheckpoint", [], |row| row.get::<_, i64>(0))
                .expect("WAL checkpoint pragma should be readable"),
            256
        );
        assert_eq!(
            connection
                .query_row("PRAGMA journal_size_limit", [], |row| row.get::<_, i64>(0))
                .expect("journal size limit should be readable"),
            1_048_576
        );
    }
}
