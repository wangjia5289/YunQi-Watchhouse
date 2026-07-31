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
    use rusqlite::{Connection, params};
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
            SchemaVersion::Inside(std::num::NonZeroUsize::new(14).expect("fourteen is non-zero"))
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

    #[test]
    fn usage_limit_reminder_migration_backfills_delivered_alerts_without_legacy_phantoms() {
        let mut connection = Connection::open_in_memory().expect("database should open");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys should be enabled");
        migrations()
            .to_version(&mut connection, 13)
            .expect("legacy schema should migrate");
        connection
            .execute(
                "INSERT INTO applications (
                    identity_key, name, first_seen_at_ms, last_seen_at_ms
                 ) VALUES ('example.browser', 'Browser', 0, 0)",
                [],
            )
            .expect("application should be inserted");
        connection
            .execute(
                "INSERT INTO applications (
                    identity_key, name, first_seen_at_ms, last_seen_at_ms
                 ) VALUES ('example.editor', 'Editor', 0, 0)",
                [],
            )
            .expect("second application should be inserted");
        connection
            .execute(
                "INSERT INTO usage_limit_rules (
                    scope_type, application_id, weekday_limit_minutes, weekend_limit_minutes,
                    created_at_ms, updated_at_ms
                 ) VALUES ('APPLICATION', 1, 60, 60, 0, 0)",
                [],
            )
            .expect("usage rule should be inserted");
        connection
            .execute(
                "INSERT INTO usage_limit_rules (
                    scope_type, application_id, weekday_limit_minutes, weekend_limit_minutes,
                    created_at_ms, updated_at_ms
                 ) VALUES ('APPLICATION', 2, 60, 60, 0, 0)",
                [],
            )
            .expect("second usage rule should be inserted");
        for (rule_id, threshold, delivered_at_ms) in
            [(1, 80, 100), (1, 100, 200), (2, 80, 300), (2, 100, 300)]
        {
            connection
                .execute(
                    "INSERT INTO usage_limit_alerts (
                        rule_id, local_date, threshold, delivered_at_ms
                     ) VALUES (?1, '2026-07-31', ?2, ?3)",
                    params![rule_id, threshold, delivered_at_ms],
                )
                .expect("legacy alert should be inserted");
        }

        migrations()
            .to_latest(&mut connection)
            .expect("current schema should migrate");
        let entries = connection
            .prepare(
                "SELECT rule_id, threshold, delivered_at_ms FROM usage_limit_reminder_history
                 ORDER BY delivered_at_ms, rule_id",
            )
            .expect("history query should prepare")
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .expect("history query should run")
            .collect::<Result<Vec<_>, _>>()
            .expect("history should decode");
        assert_eq!(entries, vec![(1, 80, 100), (1, 100, 200), (2, 100, 300)]);
    }
}
