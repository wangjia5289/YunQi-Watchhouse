use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, Row, backup::Backup, params};

use crate::{
    activity::{
        ActivitySession, ActivityState, Application, ClosedReason, NewApplication, NewSession,
    },
    error::{AppError, AppResult},
};

use super::Database;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub idle_threshold_seconds: i64,
    pub launch_at_login: bool,
    pub start_tracking_automatically: bool,
    pub hide_to_tray_on_close: bool,
    pub record_window_titles: bool,
    pub appearance: String,
    pub onboarding_completed: bool,
    pub retention_days: i64,
    pub automatic_backup_enabled: bool,
    pub backup_interval: String,
    pub backup_keep_count: i64,
    pub backup_directory: Option<String>,
    pub last_maintenance_at_ms: i64,
    pub last_backup_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenancePreview {
    pub cutoff_at_ms: Option<i64>,
    pub expired_session_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceResult {
    pub deleted_session_count: usize,
    pub deleted_application_ids: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOutcome {
    NothingToRecover,
    Closed { session_id: i64, ended_at_ms: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ActivityRecord {
    pub session: ActivitySession,
    pub application: Option<Application>,
}

#[derive(Clone)]
pub struct ActivityRepository {
    database: Database,
}

impl ActivityRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn upsert_application(&self, application: &NewApplication) -> AppResult<Application> {
        let identity_key = application.identity_key();
        let connection = self.database.lock()?;

        connection.execute(
            "INSERT INTO applications (
                identity_key, name, bundle_id, executable_path,
                first_seen_at_ms, last_seen_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(identity_key) DO UPDATE SET
                name = excluded.name,
                bundle_id = COALESCE(excluded.bundle_id, applications.bundle_id),
                executable_path = COALESCE(
                    excluded.executable_path,
                    applications.executable_path
                ),
                last_seen_at_ms = MAX(
                    applications.last_seen_at_ms,
                    excluded.last_seen_at_ms
                )",
            params![
                identity_key,
                application.name,
                application.bundle_id,
                application.executable_path,
                application.seen_at_ms,
            ],
        )?;

        connection
            .query_row(
                "SELECT id, identity_key, name, bundle_id, executable_path,
                        category, is_ignored, first_seen_at_ms, last_seen_at_ms
                 FROM applications WHERE identity_key = ?1",
                [identity_key],
                map_application,
            )
            .map_err(Into::into)
    }

    pub fn application(&self, application_id: i64) -> AppResult<Option<Application>> {
        let connection = self.database.lock()?;
        connection
            .query_row(
                "SELECT id, identity_key, name, bundle_id, executable_path,
                        category, is_ignored, first_seen_at_ms, last_seen_at_ms
                 FROM applications WHERE id = ?1",
                [application_id],
                map_application,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn is_application_ignored(&self, identity_key: &str) -> AppResult<bool> {
        let connection = self.database.lock()?;
        connection
            .query_row(
                "SELECT is_ignored FROM applications WHERE identity_key = ?1",
                [identity_key],
                |row| row.get(0),
            )
            .optional()
            .map(|value| value.unwrap_or(false))
            .map_err(Into::into)
    }

    pub fn update_application_preferences(
        &self,
        application_id: i64,
        category: &str,
        is_ignored: bool,
    ) -> AppResult<Application> {
        let category = category.trim();
        if category.is_empty() || category.chars().count() > 40 {
            return Err(AppError::InvalidSession(
                "application category must contain between 1 and 40 characters".to_owned(),
            ));
        }
        let connection = self.database.lock()?;
        let changed = connection.execute(
            "UPDATE applications SET category = ?2, is_ignored = ?3 WHERE id = ?1",
            params![application_id, category, is_ignored],
        )?;
        if changed == 0 {
            return Err(AppError::InvalidSession(format!(
                "application {application_id} was not found"
            )));
        }
        drop(connection);
        self.application(application_id)?
            .ok_or_else(|| AppError::InvalidSession("updated application was not found".to_owned()))
    }

    pub fn settings(&self) -> AppResult<Settings> {
        let connection = self.database.lock()?;
        connection
            .query_row(
                "SELECT idle_threshold_seconds, launch_at_login,
                        start_tracking_automatically, hide_to_tray_on_close,
                        record_window_titles, appearance, onboarding_completed,
                        retention_days, automatic_backup_enabled, backup_interval,
                        backup_keep_count, backup_directory, last_maintenance_at_ms,
                        last_backup_at_ms
                 FROM settings WHERE singleton_id = 1",
                [],
                |row| {
                    Ok(Settings {
                        idle_threshold_seconds: row.get(0)?,
                        launch_at_login: row.get(1)?,
                        start_tracking_automatically: row.get(2)?,
                        hide_to_tray_on_close: row.get(3)?,
                        record_window_titles: row.get(4)?,
                        appearance: row.get(5)?,
                        onboarding_completed: row.get(6)?,
                        retention_days: row.get(7)?,
                        automatic_backup_enabled: row.get(8)?,
                        backup_interval: row.get(9)?,
                        backup_keep_count: row.get(10)?,
                        backup_directory: row.get(11)?,
                        last_maintenance_at_ms: row.get(12)?,
                        last_backup_at_ms: row.get(13)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn update_settings(&self, settings: &Settings, updated_at_ms: i64) -> AppResult<Settings> {
        if !(30..=3600).contains(&settings.idle_threshold_seconds) {
            return Err(AppError::InvalidIdleThreshold(
                "must be between 30 and 3600 seconds".to_owned(),
            ));
        }
        if !matches!(settings.appearance.as_str(), "SYSTEM" | "LIGHT" | "DARK") {
            return Err(AppError::InvalidMonitorConfiguration(
                "appearance must be SYSTEM, LIGHT, or DARK".to_owned(),
            ));
        }
        if !matches!(settings.retention_days, 0 | 30 | 90 | 180 | 365) {
            return Err(AppError::InvalidMonitorConfiguration(
                "retention days must be 0, 30, 90, 180, or 365".to_owned(),
            ));
        }
        if !matches!(settings.backup_interval.as_str(), "DAILY" | "WEEKLY") {
            return Err(AppError::InvalidMonitorConfiguration(
                "backup interval must be DAILY or WEEKLY".to_owned(),
            ));
        }
        if !(1..=20).contains(&settings.backup_keep_count) {
            return Err(AppError::InvalidMonitorConfiguration(
                "backup keep count must be between 1 and 20".to_owned(),
            ));
        }
        let connection = self.database.lock()?;
        connection.execute(
            "UPDATE settings SET idle_threshold_seconds = ?1, launch_at_login = ?2,
                start_tracking_automatically = ?3, hide_to_tray_on_close = ?4,
                record_window_titles = ?5, appearance = ?6,
                retention_days = ?7, automatic_backup_enabled = ?8,
                backup_interval = ?9, backup_keep_count = ?10,
                backup_directory = ?11, updated_at_ms = ?12
             WHERE singleton_id = 1",
            params![
                settings.idle_threshold_seconds,
                settings.launch_at_login,
                settings.start_tracking_automatically,
                settings.hide_to_tray_on_close,
                settings.record_window_titles,
                settings.appearance,
                settings.retention_days,
                settings.automatic_backup_enabled,
                settings.backup_interval,
                settings.backup_keep_count,
                settings.backup_directory,
                updated_at_ms,
            ],
        )?;
        drop(connection);
        self.settings()
    }

    pub fn complete_onboarding(&self, updated_at_ms: i64) -> AppResult<Settings> {
        let connection = self.database.lock()?;
        connection.execute(
            "UPDATE settings SET onboarding_completed = 1, updated_at_ms = ?1
             WHERE singleton_id = 1",
            [updated_at_ms],
        )?;
        drop(connection);
        self.settings()
    }

    pub fn delete_all_activity(&self) -> AppResult<()> {
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM activity_sessions", [])?;
        transaction.execute("DELETE FROM applications", [])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn maintenance_preview(
        &self,
        now_ms: i64,
        retention_days: i64,
    ) -> AppResult<MaintenancePreview> {
        let cutoff_at_ms = retention_cutoff(now_ms, retention_days);
        let expired_session_count = if let Some(cutoff) = cutoff_at_ms {
            let connection = self.database.lock()?;
            connection.query_row(
                "SELECT COUNT(*) FROM activity_sessions
                 WHERE is_open = 0 AND ended_at_ms < ?1",
                [cutoff],
                |row| row.get(0),
            )?
        } else {
            0
        };
        Ok(MaintenancePreview {
            cutoff_at_ms,
            expired_session_count,
        })
    }

    pub fn run_maintenance(
        &self,
        now_ms: i64,
        retention_days: i64,
    ) -> AppResult<MaintenanceResult> {
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let deleted_session_count = if let Some(cutoff) = retention_cutoff(now_ms, retention_days) {
            transaction.execute(
                "DELETE FROM activity_sessions WHERE is_open = 0 AND ended_at_ms < ?1",
                [cutoff],
            )?
        } else {
            0
        };
        let deleted_application_ids = {
            let mut statement = transaction.prepare(
                "SELECT id FROM applications
                 WHERE is_ignored = 0 AND NOT EXISTS (
                    SELECT 1 FROM activity_sessions
                    WHERE application_id = applications.id
                 )",
            )?;
            statement
                .query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<i64>>>()?
        };
        transaction.execute(
            "DELETE FROM applications
             WHERE is_ignored = 0 AND NOT EXISTS (
                SELECT 1 FROM activity_sessions
                WHERE application_id = applications.id
             )",
            [],
        )?;
        transaction.execute(
            "UPDATE settings SET last_maintenance_at_ms = ?1 WHERE singleton_id = 1",
            [now_ms],
        )?;
        transaction.commit()?;
        Ok(MaintenanceResult {
            deleted_session_count,
            deleted_application_ids,
        })
    }

    pub fn mark_backup_completed(&self, completed_at_ms: i64) -> AppResult<()> {
        let connection = self.database.lock()?;
        connection.execute(
            "UPDATE settings SET last_backup_at_ms = ?1 WHERE singleton_id = 1",
            [completed_at_ms],
        )?;
        Ok(())
    }

    pub fn all_activity_records(&self) -> AppResult<Vec<ActivityRecord>> {
        self.records_overlapping(i64::MIN, i64::MAX)
    }

    pub fn delete_closed_session(&self, session_id: i64) -> AppResult<()> {
        let connection = self.database.lock()?;
        let session =
            find_session(&connection, session_id)?.ok_or(AppError::SessionNotFound(session_id))?;
        if session.is_open {
            return Err(AppError::InvalidSession(
                "the currently open session cannot be deleted".to_owned(),
            ));
        }
        connection.execute("DELETE FROM activity_sessions WHERE id = ?1", [session_id])?;
        Ok(())
    }

    pub fn update_closed_session_bounds(
        &self,
        session_id: i64,
        started_at_ms: i64,
        ended_at_ms: i64,
    ) -> AppResult<ActivitySession> {
        if ended_at_ms <= started_at_ms {
            return Err(AppError::InvalidTimeRange(
                "session end must be after its start".to_owned(),
            ));
        }
        let connection = self.database.lock()?;
        let overlaps_another: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM activity_sessions
                WHERE id != ?1 AND started_at_ms < ?3 AND ended_at_ms > ?2
            )",
            params![session_id, started_at_ms, ended_at_ms],
            |row| row.get(0),
        )?;
        if overlaps_another {
            return Err(AppError::InvalidTimeRange(
                "edited session cannot overlap another session".to_owned(),
            ));
        }
        let changed = connection.execute(
            "UPDATE activity_sessions
             SET started_at_ms = ?2, ended_at_ms = ?3,
                 duration_ms = ?3 - ?2, updated_at_ms = ?3
             WHERE id = ?1 AND is_open = 0",
            params![session_id, started_at_ms, ended_at_ms],
        )?;
        if changed == 0 {
            return Err(AppError::InvalidSession(
                "only closed sessions can be edited".to_owned(),
            ));
        }
        find_session(&connection, session_id)?.ok_or(AppError::SessionNotFound(session_id))
    }

    pub fn backup_database(&self, destination: &Path) -> AppResult<()> {
        let source = self.database.lock()?;
        let mut destination = Connection::open(destination)?;
        Backup::new(&source, &mut destination)?.run_to_completion(
            64,
            Duration::from_millis(10),
            None,
        )?;
        Ok(())
    }

    pub fn restore_database(&self, source_path: &Path) -> AppResult<()> {
        let source =
            Connection::open_with_flags(source_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut staging = Connection::open_in_memory()?;
        Backup::new(&source, &mut staging)?.run_to_completion(
            64,
            Duration::from_millis(10),
            None,
        )?;

        let integrity: String =
            staging.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(AppError::InvalidSession(format!(
                "backup database failed integrity check: {integrity}"
            )));
        }
        for table in ["applications", "activity_sessions", "settings"] {
            let exists: bool = staging.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::InvalidSession(format!(
                    "backup database is missing table {table}"
                )));
            }
        }
        super::migration::migrations().to_latest(&mut staging)?;

        let mut destination = self.database.lock()?;
        Backup::new(&staging, &mut destination)?.run_to_completion(
            64,
            Duration::from_millis(10),
            None,
        )?;
        Ok(())
    }

    pub fn optimize_database(&self) -> AppResult<()> {
        let connection = self.database.lock()?;
        connection.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);
             VACUUM;
             PRAGMA optimize;",
        )?;
        Ok(())
    }

    pub fn record_counts(&self) -> AppResult<(i64, i64)> {
        let connection = self.database.lock()?;
        let applications =
            connection.query_row("SELECT COUNT(*) FROM applications", [], |row| row.get(0))?;
        let sessions =
            connection.query_row("SELECT COUNT(*) FROM activity_sessions", [], |row| {
                row.get(0)
            })?;
        Ok((applications, sessions))
    }

    pub fn create_session(&self, session: &NewSession) -> AppResult<ActivitySession> {
        validate_new_session(session)?;
        let connection = self.database.lock()?;

        connection.execute(
            "INSERT INTO activity_sessions (
                state, application_id, window_title, started_at_ms, ended_at_ms,
                duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?4, 0, 1, NULL, ?4, ?4)",
            params![
                session.state.as_db_str(),
                session.application_id,
                session.window_title,
                session.started_at_ms,
            ],
        )?;

        find_session(&connection, connection.last_insert_rowid())?
            .ok_or_else(|| AppError::SessionNotFound(connection.last_insert_rowid()))
    }

    pub fn checkpoint_session(
        &self,
        session_id: i64,
        ended_at_ms: i64,
    ) -> AppResult<ActivitySession> {
        let connection = self.database.lock()?;
        let changed = connection.execute(
            "UPDATE activity_sessions
             SET ended_at_ms = ?2,
                 duration_ms = ?2 - started_at_ms,
                 updated_at_ms = ?2
             WHERE id = ?1 AND is_open = 1 AND ?2 >= ended_at_ms",
            params![session_id, ended_at_ms],
        )?;

        if changed == 0 {
            return Err(AppError::SessionNotFound(session_id));
        }

        find_session(&connection, session_id)?.ok_or(AppError::SessionNotFound(session_id))
    }

    pub fn close_session(
        &self,
        session_id: i64,
        ended_at_ms: i64,
        reason: ClosedReason,
    ) -> AppResult<ActivitySession> {
        let connection = self.database.lock()?;
        let changed = connection.execute(
            "UPDATE activity_sessions
             SET ended_at_ms = ?2,
                 duration_ms = ?2 - started_at_ms,
                 is_open = 0,
                 closed_reason = ?3,
                 updated_at_ms = ?2
             WHERE id = ?1 AND is_open = 1 AND ?2 >= started_at_ms",
            params![session_id, ended_at_ms, reason.as_db_str()],
        )?;

        if changed == 0 {
            return Err(AppError::SessionNotFound(session_id));
        }

        find_session(&connection, session_id)?.ok_or(AppError::SessionNotFound(session_id))
    }

    pub fn transition_session(
        &self,
        current_session_id: i64,
        ended_at_ms: i64,
        reason: ClosedReason,
        next_session: &NewSession,
    ) -> AppResult<ActivitySession> {
        validate_new_session(next_session)?;
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;

        let changed = transaction.execute(
            "UPDATE activity_sessions
             SET ended_at_ms = ?2,
                 duration_ms = ?2 - started_at_ms,
                 is_open = 0,
                 closed_reason = ?3,
                 updated_at_ms = ?2
             WHERE id = ?1 AND is_open = 1 AND ?2 >= started_at_ms",
            params![current_session_id, ended_at_ms, reason.as_db_str()],
        )?;
        if changed == 0 {
            return Err(AppError::SessionNotFound(current_session_id));
        }

        transaction.execute(
            "INSERT INTO activity_sessions (
                state, application_id, window_title, started_at_ms, ended_at_ms,
                duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?4, 0, 1, NULL, ?4, ?4)",
            params![
                next_session.state.as_db_str(),
                next_session.application_id,
                next_session.window_title,
                next_session.started_at_ms,
            ],
        )?;
        let next_session_id = transaction.last_insert_rowid();
        transaction.commit()?;

        find_session(&connection, next_session_id)?
            .ok_or(AppError::SessionNotFound(next_session_id))
    }

    pub fn open_session(&self) -> AppResult<Option<ActivitySession>> {
        let connection = self.database.lock()?;
        connection
            .query_row(
                &format!("{SESSION_SELECT} WHERE is_open = 1"),
                [],
                map_session,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn sessions_overlapping(
        &self,
        range_start_ms: i64,
        range_end_ms: i64,
    ) -> AppResult<Vec<ActivitySession>> {
        if range_end_ms <= range_start_ms {
            return Err(AppError::InvalidSession(
                "query range end must be after its start".to_owned(),
            ));
        }

        let connection = self.database.lock()?;
        let mut statement = connection.prepare(&format!(
            "{SESSION_SELECT}
             WHERE started_at_ms < ?2 AND ended_at_ms > ?1
             ORDER BY started_at_ms, id"
        ))?;
        let sessions = statement
            .query_map(params![range_start_ms, range_end_ms], map_session)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(sessions)
    }

    pub fn records_overlapping(
        &self,
        range_start_ms: i64,
        range_end_ms: i64,
    ) -> AppResult<Vec<ActivityRecord>> {
        if range_end_ms <= range_start_ms {
            return Err(AppError::InvalidTimeRange(
                "range end must be after range start".to_owned(),
            ));
        }

        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT
                s.id, s.state, s.application_id, s.window_title,
                s.started_at_ms, s.ended_at_ms, s.duration_ms, s.is_open,
                s.closed_reason, s.created_at_ms, s.updated_at_ms,
                a.id, a.identity_key, a.name, a.bundle_id, a.executable_path,
                a.category, a.is_ignored, a.first_seen_at_ms, a.last_seen_at_ms
             FROM activity_sessions s
             LEFT JOIN applications a ON a.id = s.application_id
             WHERE s.started_at_ms < ?2 AND s.ended_at_ms > ?1
             ORDER BY s.started_at_ms, s.id",
        )?;
        let records = statement
            .query_map(params![range_start_ms, range_end_ms], |row| {
                let session = map_session(row)?;
                let application_id: Option<i64> = row.get(11)?;
                let application = application_id
                    .map(|id| {
                        Ok::<Application, rusqlite::Error>(Application {
                            id,
                            identity_key: row.get(12)?,
                            name: row.get(13)?,
                            bundle_id: row.get(14)?,
                            executable_path: row.get(15)?,
                            category: row.get(16)?,
                            is_ignored: row.get(17)?,
                            first_seen_at_ms: row.get(18)?,
                            last_seen_at_ms: row.get(19)?,
                        })
                    })
                    .transpose()?;
                Ok(ActivityRecord {
                    session,
                    application,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(records)
    }

    pub fn recover_open_session(&self) -> AppResult<RecoveryOutcome> {
        let Some(session) = self.open_session()? else {
            return Ok(RecoveryOutcome::NothingToRecover);
        };

        // `ended_at_ms` is the last durable checkpoint. Time after it is
        // unknowable after a crash and must not be assigned to the old app.
        self.close_session(session.id, session.ended_at_ms, ClosedReason::CrashRecovery)?;

        Ok(RecoveryOutcome::Closed {
            session_id: session.id,
            ended_at_ms: session.ended_at_ms,
        })
    }
}

const SESSION_SELECT: &str = "SELECT
    id, state, application_id, window_title, started_at_ms, ended_at_ms,
    duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms
    FROM activity_sessions";

fn validate_new_session(session: &NewSession) -> AppResult<()> {
    match (session.state, session.application_id) {
        (ActivityState::Active, None) => Err(AppError::InvalidSession(
            "ACTIVE sessions require an application".to_owned(),
        )),
        (ActivityState::Idle, Some(_)) => Err(AppError::InvalidSession(
            "IDLE sessions cannot reference an application".to_owned(),
        )),
        _ => Ok(()),
    }
}

fn retention_cutoff(now_ms: i64, retention_days: i64) -> Option<i64> {
    if retention_days == 0 {
        None
    } else {
        Some(now_ms.saturating_sub(retention_days.saturating_mul(24 * 60 * 60 * 1_000)))
    }
}

fn find_session(
    connection: &rusqlite::Connection,
    session_id: i64,
) -> AppResult<Option<ActivitySession>> {
    connection
        .query_row(
            &format!("{SESSION_SELECT} WHERE id = ?1"),
            [session_id],
            map_session,
        )
        .optional()
        .map_err(Into::into)
}

fn map_application(row: &Row<'_>) -> rusqlite::Result<Application> {
    Ok(Application {
        id: row.get(0)?,
        identity_key: row.get(1)?,
        name: row.get(2)?,
        bundle_id: row.get(3)?,
        executable_path: row.get(4)?,
        category: row.get(5)?,
        is_ignored: row.get(6)?,
        first_seen_at_ms: row.get(7)?,
        last_seen_at_ms: row.get(8)?,
    })
}

fn map_session(row: &Row<'_>) -> rusqlite::Result<ActivitySession> {
    let state: String = row.get(1)?;
    let closed_reason: Option<String> = row.get(8)?;

    Ok(ActivitySession {
        id: row.get(0)?,
        state: ActivityState::from_db_str(&state)?,
        application_id: row.get(2)?,
        window_title: row.get(3)?,
        started_at_ms: row.get(4)?,
        ended_at_ms: row.get(5)?,
        duration_ms: row.get(6)?,
        is_open: row.get(7)?,
        closed_reason: closed_reason
            .as_deref()
            .map(ClosedReason::from_db_str)
            .transpose()?,
        created_at_ms: row.get(9)?,
        updated_at_ms: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> ActivityRepository {
        ActivityRepository::new(Database::in_memory().expect("database should open"))
    }

    fn application(repository: &ActivityRepository, seen_at_ms: i64) -> Application {
        repository
            .upsert_application(&NewApplication {
                name: "IntelliJ IDEA".to_owned(),
                bundle_id: Some("com.jetbrains.intellij".to_owned()),
                executable_path: Some("/Applications/IntelliJ IDEA.app".to_owned()),
                seen_at_ms,
            })
            .expect("application should be stored")
    }

    #[test]
    fn application_upsert_preserves_first_seen_and_updates_last_seen() {
        let repository = repository();
        let first = application(&repository, 100);
        let second = application(&repository, 250);

        assert_eq!(first.id, second.id);
        assert_eq!(second.first_seen_at_ms, 100);
        assert_eq!(second.last_seen_at_ms, 250);
    }

    #[test]
    fn onboarding_starts_incomplete_and_can_be_completed() {
        let repository = repository();
        assert!(
            !repository
                .settings()
                .expect("settings should load")
                .onboarding_completed
        );

        let settings = repository
            .complete_onboarding(123)
            .expect("onboarding should complete");
        assert!(settings.onboarding_completed);
        assert!(
            repository
                .settings()
                .expect("settings should reload")
                .onboarding_completed
        );
    }

    #[test]
    fn active_session_requires_an_application() {
        let error = repository()
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: None,
                window_title: None,
                started_at_ms: 100,
            })
            .expect_err("invalid session should fail");

        assert!(matches!(error, AppError::InvalidSession(_)));
    }

    #[test]
    fn only_one_session_can_be_open() {
        let repository = repository();
        repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                started_at_ms: 100,
            })
            .expect("first session should open");

        let error = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                started_at_ms: 200,
            })
            .expect_err("second open session should fail");

        assert!(matches!(error, AppError::Database(_)));
    }

    #[test]
    fn checkpoint_and_close_keep_duration_consistent() {
        let repository = repository();
        let app = application(&repository, 100);
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                started_at_ms: 100,
            })
            .expect("session should open");

        let checkpoint = repository
            .checkpoint_session(session.id, 175)
            .expect("checkpoint should succeed");
        assert_eq!(checkpoint.duration_ms, 75);
        assert!(checkpoint.is_open);

        let closed = repository
            .close_session(session.id, 225, ClosedReason::AppChanged)
            .expect("close should succeed");
        assert_eq!(closed.duration_ms, 125);
        assert_eq!(closed.closed_reason, Some(ClosedReason::AppChanged));
        assert!(!closed.is_open);
    }

    #[test]
    fn application_preferences_and_closed_session_bounds_can_be_updated() {
        let repository = repository();
        let app = application(&repository, 100);
        let preferences = repository
            .update_application_preferences(app.id, "Work", true)
            .expect("preferences should update");
        assert_eq!(preferences.category, "Work");
        assert!(preferences.is_ignored);

        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                started_at_ms: 100,
            })
            .expect("session should open");
        assert!(
            repository
                .update_closed_session_bounds(session.id, 90, 210)
                .is_err()
        );
        repository
            .close_session(session.id, 200, ClosedReason::AppChanged)
            .expect("session should close");
        let updated = repository
            .update_closed_session_bounds(session.id, 90, 210)
            .expect("closed session should update");
        assert_eq!((updated.started_at_ms, updated.ended_at_ms), (90, 210));
        assert_eq!(updated.duration_ms, 120);
    }

    #[test]
    fn maintenance_deletes_only_expired_closed_sessions_and_preserves_ignore_rules() {
        let repository = repository();
        let app = application(&repository, 100);
        let old = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                started_at_ms: 100,
            })
            .expect("old session should open");
        repository
            .close_session(old.id, 200, ClosedReason::AppChanged)
            .expect("old session should close");
        let recent = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                started_at_ms: 9_000,
            })
            .expect("recent session should open");
        repository
            .close_session(recent.id, 9_500, ClosedReason::AppChanged)
            .expect("recent session should close");

        let ignored = repository
            .upsert_application(&NewApplication {
                name: "Private".to_owned(),
                bundle_id: Some("example.private".to_owned()),
                executable_path: None,
                seen_at_ms: 0,
            })
            .expect("ignored application should be stored");
        repository
            .update_application_preferences(ignored.id, "Personal", true)
            .expect("ignore rule should be stored");

        let preview = repository
            .maintenance_preview(10_000, 1)
            .expect("preview should succeed");
        assert_eq!(preview.expired_session_count, 0);

        let result = repository
            .run_maintenance(10_000, 30)
            .expect("maintenance should succeed");
        assert_eq!(result.deleted_session_count, 0);
        assert!(
            repository
                .application(ignored.id)
                .expect("ignored application should load")
                .is_some()
        );

        let result = repository
            .run_maintenance(100 * 24 * 60 * 60 * 1_000, 30)
            .expect("expired maintenance should succeed");
        assert_eq!(result.deleted_session_count, 2);
        assert!(
            repository
                .application(ignored.id)
                .expect("ignored application should load")
                .is_some()
        );
    }

    #[test]
    fn permanent_retention_never_marks_sessions_for_deletion() {
        let repository = repository();
        let app = application(&repository, 0);
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                started_at_ms: 0,
            })
            .expect("session should open");
        repository
            .close_session(session.id, 100, ClosedReason::AppChanged)
            .expect("session should close");

        let preview = repository
            .maintenance_preview(i64::MAX, 0)
            .expect("preview should succeed");
        assert_eq!(preview.cutoff_at_ms, None);
        assert_eq!(preview.expired_session_count, 0);
    }

    #[test]
    fn overlap_query_uses_half_open_range_boundaries() {
        let repository = repository();
        let app = application(&repository, 0);

        for (start, end) in [(0, 100), (100, 200), (200, 300)] {
            let session = repository
                .create_session(&NewSession {
                    state: ActivityState::Active,
                    application_id: Some(app.id),
                    window_title: None,
                    started_at_ms: start,
                })
                .expect("session should open");
            repository
                .close_session(session.id, end, ClosedReason::AppChanged)
                .expect("session should close");
        }

        let sessions = repository
            .sessions_overlapping(100, 200)
            .expect("query should succeed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].started_at_ms, 100);
    }

    #[test]
    fn recovery_closes_at_last_durable_checkpoint() {
        let repository = repository();
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                started_at_ms: 100,
            })
            .expect("session should open");
        repository
            .checkpoint_session(session.id, 180)
            .expect("checkpoint should succeed");

        assert_eq!(
            repository
                .recover_open_session()
                .expect("recovery should succeed"),
            RecoveryOutcome::Closed {
                session_id: session.id,
                ended_at_ms: 180,
            }
        );
        let recovered = repository
            .sessions_overlapping(100, 181)
            .expect("query should succeed")
            .pop()
            .expect("recovered session should exist");
        assert_eq!(recovered.duration_ms, 80);
        assert_eq!(recovered.closed_reason, Some(ClosedReason::CrashRecovery));
    }

    #[test]
    fn database_backup_and_restore_round_trip_activity() {
        let repository = repository();
        let app = application(&repository, 100);
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                started_at_ms: 100,
            })
            .expect("session should open");
        repository
            .close_session(session.id, 250, ClosedReason::AppChanged)
            .expect("session should close");

        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let backup = directory.path().join("watchhouse-backup.sqlite3");
        repository
            .backup_database(&backup)
            .expect("database should back up");
        repository
            .delete_all_activity()
            .expect("activity should be deleted");
        assert!(
            repository
                .records_overlapping(0, 300)
                .expect("query should succeed")
                .is_empty()
        );

        repository
            .restore_database(&backup)
            .expect("database should restore");
        let records = repository
            .records_overlapping(0, 300)
            .expect("query should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].session.duration_ms, 150);
        assert_eq!(
            records[0].application.as_ref().map(|app| app.name.as_str()),
            Some("IntelliJ IDEA")
        );
    }

    #[test]
    fn restore_rejects_a_non_watchhouse_database() {
        let repository = repository();
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let invalid = directory.path().join("invalid.sqlite3");
        Connection::open(&invalid).expect("invalid database should be created");

        assert!(matches!(
            repository.restore_database(&invalid),
            Err(AppError::InvalidSession(_))
        ));
    }

    #[test]
    fn restore_migrates_a_legacy_database_before_replacing_current_data() {
        let repository = repository();
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let legacy = directory.path().join("legacy.sqlite3");
        let mut connection = Connection::open(&legacy).expect("legacy database should open");
        super::super::migration::migrations()
            .to_version(&mut connection, 1)
            .expect("legacy schema should be created");
        drop(connection);

        repository
            .restore_database(&legacy)
            .expect("legacy database should migrate and restore");
        assert!(
            !repository
                .settings()
                .expect("migrated settings should load")
                .onboarding_completed
        );
    }

    #[test]
    fn closed_session_can_be_deleted_without_deleting_its_application() {
        let repository = repository();
        let app = application(&repository, 100);
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                started_at_ms: 100,
            })
            .expect("session should open");
        repository
            .close_session(session.id, 200, ClosedReason::AppChanged)
            .expect("session should close");

        repository
            .delete_closed_session(session.id)
            .expect("closed session should delete");
        assert!(
            repository
                .records_overlapping(0, 300)
                .expect("query should succeed")
                .is_empty()
        );
        assert!(
            repository
                .application(app.id)
                .expect("application query should succeed")
                .is_some()
        );
    }

    #[test]
    fn open_session_cannot_be_deleted() {
        let repository = repository();
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                started_at_ms: 100,
            })
            .expect("session should open");

        assert!(matches!(
            repository.delete_closed_session(session.id),
            Err(AppError::InvalidSession(_))
        ));
    }

    #[test]
    fn record_counts_report_applications_and_sessions() {
        let repository = repository();
        let app = application(&repository, 100);
        repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                started_at_ms: 100,
            })
            .expect("session should open");

        assert_eq!(
            repository.record_counts().expect("counts should load"),
            (1, 1)
        );
    }
}
