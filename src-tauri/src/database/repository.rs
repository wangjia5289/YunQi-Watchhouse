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
    pub daily_focus_goal_minutes: i64,
    pub focus_block_gap_minutes: i64,
    pub break_reminders_enabled: bool,
    pub break_reminder_minutes: i64,
    pub quiet_hours_start: String,
    pub quiet_hours_end: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSettings {
    pub toggle_focus: Option<String>,
    pub pause_focus: Option<String>,
    pub start_template: Option<String>,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSearch {
    pub query: Option<String>,
    pub state: Option<ActivityState>,
    pub minimum_duration_ms: Option<i64>,
    pub maximum_duration_ms: Option<i64>,
    pub time_from_minutes: Option<i64>,
    pub time_to_minutes: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRecord {
    pub state: ActivityState,
    pub application_name: Option<String>,
    pub bundle_identifier: Option<String>,
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
    pub window_title: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineMutationResult {
    pub affected_count: usize,
    pub undo_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineUndoEntry {
    pub token: String,
    pub created_at_ms: i64,
    pub session_count: usize,
    pub operation_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusPlanHistoryEntry {
    pub id: i64,
    pub started_at_ms: i64,
    pub planned_end_at_ms: Option<i64>,
    pub ended_at_ms: i64,
    pub paused_duration_ms: i64,
    pub outcome: String,
    pub template_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusPlanTemplate {
    pub id: i64,
    pub name: String,
    pub duration_minutes: i64,
    pub sort_order: i64,
    pub use_count: i64,
    pub completed_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataHealthSummary {
    pub overlapping_session_count: i64,
    pub zero_duration_session_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataHealthRepairResult {
    pub trimmed_session_count: usize,
    pub deleted_session_count: usize,
    pub backup_path: String,
    pub undo_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataHealthUndoStatus {
    pub available: bool,
    pub backup_path: Option<String>,
    pub created_at_ms: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DataHealthUndoSnapshot {
    sessions: Vec<ActivitySession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedFocusMode {
    pub active: bool,
    pub started_at_ms: Option<i64>,
    pub planned_end_at_ms: Option<i64>,
    pub paused: bool,
    pub paused_at_ms: Option<i64>,
    pub total_paused_ms: i64,
    pub template_id: Option<i64>,
}

type OverlappingSession = (i64, String, Option<i64>, Option<String>, i64, i64);

#[derive(serde::Serialize, serde::Deserialize)]
struct TimelineUndoSnapshot {
    sessions: Vec<ActivitySession>,
    delete_session_ids: Vec<i64>,
    #[serde(default)]
    operation_label: Option<String>,
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
                        category, is_ignored, record_window_titles,
                        first_seen_at_ms, last_seen_at_ms
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
                        category, is_ignored, record_window_titles,
                        first_seen_at_ms, last_seen_at_ms
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
        record_window_titles: bool,
    ) -> AppResult<Application> {
        let category = category.trim();
        if category.is_empty() || category.chars().count() > 40 {
            return Err(AppError::InvalidSession(
                "application category must contain between 1 and 40 characters".to_owned(),
            ));
        }
        let connection = self.database.lock()?;
        let changed = connection.execute(
            "UPDATE applications
             SET category = ?2, is_ignored = ?3, record_window_titles = ?4
             WHERE id = ?1",
            params![application_id, category, is_ignored, record_window_titles],
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

    pub fn should_record_window_title(&self, identity_key: &str) -> AppResult<bool> {
        let connection = self.database.lock()?;
        connection
            .query_row(
                "SELECT s.record_window_titles AND COALESCE(a.record_window_titles, 0)
                 FROM settings s
                 LEFT JOIN applications a ON a.identity_key = ?1
                 WHERE s.singleton_id = 1",
                [identity_key],
                |row| row.get(0),
            )
            .map_err(Into::into)
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
                        last_backup_at_ms, daily_focus_goal_minutes,
                        focus_block_gap_minutes, break_reminders_enabled,
                        break_reminder_minutes, quiet_hours_start, quiet_hours_end
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
                        daily_focus_goal_minutes: row.get(14)?,
                        focus_block_gap_minutes: row.get(15)?,
                        break_reminders_enabled: row.get(16)?,
                        break_reminder_minutes: row.get(17)?,
                        quiet_hours_start: row.get(18)?,
                        quiet_hours_end: row.get(19)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn shortcut_settings(&self) -> AppResult<ShortcutSettings> {
        let connection = self.database.lock()?;
        connection
            .query_row(
                "SELECT toggle_focus, pause_focus, start_template
                 FROM shortcut_settings WHERE singleton_id = 1",
                [],
                |row| {
                    Ok(ShortcutSettings {
                        toggle_focus: row.get(0)?,
                        pause_focus: row.get(1)?,
                        start_template: row.get(2)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn update_shortcut_settings(
        &self,
        settings: &ShortcutSettings,
        updated_at_ms: i64,
    ) -> AppResult<ShortcutSettings> {
        let connection = self.database.lock()?;
        connection.execute(
            "UPDATE shortcut_settings
             SET toggle_focus = ?1, pause_focus = ?2, start_template = ?3, updated_at_ms = ?4
             WHERE singleton_id = 1",
            params![
                settings.toggle_focus,
                settings.pause_focus,
                settings.start_template,
                updated_at_ms
            ],
        )?;
        drop(connection);
        self.shortcut_settings()
    }

    pub fn focus_mode_status(&self) -> AppResult<PersistedFocusMode> {
        let connection = self.database.lock()?;
        connection
            .query_row(
                "SELECT focus_mode_active, focus_mode_started_at_ms,
                        focus_plan_end_at_ms, focus_plan_paused,
                        focus_plan_paused_at_ms, focus_plan_total_paused_ms,
                        focus_plan_template_id
                 FROM settings WHERE singleton_id = 1",
                [],
                |row| {
                    Ok(PersistedFocusMode {
                        active: row.get(0)?,
                        started_at_ms: row.get(1)?,
                        planned_end_at_ms: row.get(2)?,
                        paused: row.get(3)?,
                        paused_at_ms: row.get(4)?,
                        total_paused_ms: row.get(5)?,
                        template_id: row.get(6)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn update_focus_mode(
        &self,
        status: &PersistedFocusMode,
        updated_at_ms: i64,
    ) -> AppResult<()> {
        let connection = self.database.lock()?;
        connection.execute(
            "UPDATE settings
             SET focus_mode_active = ?1, focus_mode_started_at_ms = ?2,
                 focus_plan_end_at_ms = ?3, focus_plan_paused = ?4,
                 focus_plan_paused_at_ms = ?5, focus_plan_total_paused_ms = ?6,
                 focus_plan_template_id = ?7, updated_at_ms = ?8
             WHERE singleton_id = 1",
            params![
                status.active,
                status.active.then_some(status.started_at_ms).flatten(),
                status.active.then_some(status.planned_end_at_ms).flatten(),
                status.active && status.paused,
                (status.active && status.paused)
                    .then_some(status.paused_at_ms)
                    .flatten(),
                if status.active {
                    status.total_paused_ms
                } else {
                    0
                },
                status.active.then_some(status.template_id).flatten(),
                updated_at_ms
            ],
        )?;
        Ok(())
    }

    pub fn record_focus_plan_outcome(
        &self,
        started_at_ms: i64,
        planned_end_at_ms: Option<i64>,
        ended_at_ms: i64,
        paused_duration_ms: i64,
        completed: bool,
        template_id: Option<i64>,
    ) -> AppResult<()> {
        let connection = self.database.lock()?;
        connection.execute(
            "INSERT INTO focus_plan_history (
                started_at_ms, planned_end_at_ms, ended_at_ms,
                paused_duration_ms, outcome, template_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                started_at_ms,
                planned_end_at_ms,
                ended_at_ms,
                paused_duration_ms,
                if completed { "COMPLETED" } else { "CANCELLED" },
                template_id,
            ],
        )?;
        if completed && let Some(template_id) = template_id {
            connection.execute(
                "UPDATE focus_plan_templates
                 SET completed_count = completed_count + 1, updated_at_ms = ?2
                 WHERE id = ?1",
                params![template_id, ended_at_ms],
            )?;
        }
        Ok(())
    }

    pub fn focus_plan_history(
        &self,
        range_start_ms: i64,
        range_end_ms: i64,
    ) -> AppResult<Vec<FocusPlanHistoryEntry>> {
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, started_at_ms, planned_end_at_ms, ended_at_ms,
                    paused_duration_ms, outcome, template_id
             FROM focus_plan_history
             WHERE ended_at_ms >= ?1 AND ended_at_ms < ?2
             ORDER BY ended_at_ms DESC, id DESC",
        )?;
        let entries = statement
            .query_map(params![range_start_ms, range_end_ms], |row| {
                Ok(FocusPlanHistoryEntry {
                    id: row.get(0)?,
                    started_at_ms: row.get(1)?,
                    planned_end_at_ms: row.get(2)?,
                    ended_at_ms: row.get(3)?,
                    paused_duration_ms: row.get(4)?,
                    outcome: row.get(5)?,
                    template_id: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    pub fn focus_plan_templates(&self) -> AppResult<Vec<FocusPlanTemplate>> {
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, name, duration_minutes, sort_order, use_count, completed_count
             FROM focus_plan_templates ORDER BY sort_order, created_at_ms, id",
        )?;
        Ok(statement
            .query_map([], |row| {
                Ok(FocusPlanTemplate {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    duration_minutes: row.get(2)?,
                    sort_order: row.get(3)?,
                    use_count: row.get(4)?,
                    completed_count: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn create_focus_plan_template(
        &self,
        name: &str,
        duration_minutes: i64,
    ) -> AppResult<FocusPlanTemplate> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 || !(5..=240).contains(&duration_minutes) {
            return Err(AppError::InvalidSession(
                "template name must be 1-40 characters and duration 5-240 minutes".to_owned(),
            ));
        }
        let connection = self.database.lock()?;
        let now = now_millis();
        let sort_order: i64 = connection.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM focus_plan_templates",
            [],
            |row| row.get(0),
        )?;
        connection.execute(
            "INSERT INTO focus_plan_templates(
               name, duration_minutes, created_at_ms, updated_at_ms, sort_order
             ) VALUES (?1, ?2, ?3, ?3, ?4)",
            params![name, duration_minutes, now, sort_order],
        )?;
        Ok(FocusPlanTemplate {
            id: connection.last_insert_rowid(),
            name: name.to_owned(),
            duration_minutes,
            sort_order,
            use_count: 0,
            completed_count: 0,
        })
    }

    pub fn update_focus_plan_template(
        &self,
        id: i64,
        name: &str,
        duration_minutes: i64,
        sort_order: i64,
    ) -> AppResult<FocusPlanTemplate> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 || !(5..=240).contains(&duration_minutes) {
            return Err(AppError::InvalidSession(
                "template name must be 1-40 characters and duration 5-240 minutes".to_owned(),
            ));
        }
        let connection = self.database.lock()?;
        connection.execute(
            "UPDATE focus_plan_templates
             SET name = ?2, duration_minutes = ?3, sort_order = ?4, updated_at_ms = ?5
             WHERE id = ?1",
            params![id, name, duration_minutes, sort_order, now_millis()],
        )?;
        connection
            .query_row(
                "SELECT id, name, duration_minutes, sort_order, use_count, completed_count
                 FROM focus_plan_templates WHERE id = ?1",
                [id],
                |row| {
                    Ok(FocusPlanTemplate {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        duration_minutes: row.get(2)?,
                        sort_order: row.get(3)?,
                        use_count: row.get(4)?,
                        completed_count: row.get(5)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn mark_focus_template_started(&self, id: i64, now_ms: i64) -> AppResult<()> {
        let connection = self.database.lock()?;
        let changed = connection.execute(
            "UPDATE focus_plan_templates
             SET use_count = use_count + 1, updated_at_ms = ?2 WHERE id = ?1",
            params![id, now_ms],
        )?;
        if changed == 0 {
            return Err(AppError::InvalidSession(
                "focus template was not found".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn delete_focus_plan_template(&self, id: i64) -> AppResult<()> {
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE settings SET focus_plan_template_id = NULL
             WHERE focus_plan_template_id = ?1",
            [id],
        )?;
        transaction.execute(
            "UPDATE focus_plan_history SET template_id = NULL WHERE template_id = ?1",
            [id],
        )?;
        transaction.execute("DELETE FROM focus_plan_templates WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn data_health_summary(&self) -> AppResult<DataHealthSummary> {
        let connection = self.database.lock()?;
        let overlapping_session_count = connection.query_row(
            "SELECT COUNT(*) FROM activity_sessions current
             WHERE current.is_open = 0 AND EXISTS (
               SELECT 1 FROM activity_sessions next
               WHERE next.is_open = 0 AND next.id != current.id
                 AND next.started_at_ms < current.ended_at_ms
                 AND next.ended_at_ms > current.started_at_ms
             )",
            [],
            |row| row.get(0),
        )?;
        let zero_duration_session_count = connection.query_row(
            "SELECT COUNT(*) FROM activity_sessions WHERE is_open = 0 AND duration_ms = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(DataHealthSummary {
            overlapping_session_count,
            zero_duration_session_count,
        })
    }

    pub fn repair_data_health(&self, backup_path: &str) -> AppResult<DataHealthRepairResult> {
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let mut snapshot_statement = transaction.prepare(&format!(
            "{SESSION_SELECT}
             WHERE is_open = 0 AND (
               duration_ms = 0 OR EXISTS (
                 SELECT 1 FROM activity_sessions other
                 WHERE other.is_open = 0 AND other.id != activity_sessions.id
                   AND other.started_at_ms < activity_sessions.ended_at_ms
                   AND other.ended_at_ms > activity_sessions.started_at_ms
               )
             ) ORDER BY started_at_ms, id"
        ))?;
        let snapshot = snapshot_statement
            .query_map([], map_session)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(snapshot_statement);
        transaction.execute(
            "INSERT INTO data_health_undo(singleton_id, snapshot_json, backup_path, created_at_ms)
             VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(singleton_id) DO UPDATE SET
               snapshot_json = excluded.snapshot_json,
               backup_path = excluded.backup_path,
               created_at_ms = excluded.created_at_ms",
            params![
                serde_json::to_string(&DataHealthUndoSnapshot { sessions: snapshot }).map_err(
                    |error| AppError::InvalidSession(format!(
                        "could not create health repair snapshot: {error}"
                    ))
                )?,
                backup_path,
                now_millis(),
            ],
        )?;
        let mut deleted_session_count = transaction.execute(
            "DELETE FROM activity_sessions WHERE is_open = 0 AND duration_ms = 0",
            [],
        )?;
        let mut statement = transaction.prepare(
            "SELECT id, started_at_ms, ended_at_ms FROM activity_sessions
             WHERE is_open = 0 ORDER BY started_at_ms, ended_at_ms, id",
        )?;
        let sessions = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut trimmed_session_count = 0;
        for pair in sessions.windows(2) {
            let (id, started_at_ms, ended_at_ms) = pair[0];
            let next_start = pair[1].1;
            if ended_at_ms > next_start && next_start > started_at_ms {
                trimmed_session_count += transaction.execute(
                    "UPDATE activity_sessions
                     SET ended_at_ms = ?2, duration_ms = ?2 - started_at_ms, updated_at_ms = ?2
                     WHERE id = ?1",
                    params![id, next_start],
                )?;
            } else if ended_at_ms > next_start {
                deleted_session_count +=
                    transaction.execute("DELETE FROM activity_sessions WHERE id = ?1", [id])?;
            }
        }
        transaction.commit()?;
        Ok(DataHealthRepairResult {
            trimmed_session_count,
            deleted_session_count,
            backup_path: backup_path.to_owned(),
            undo_available: true,
        })
    }

    pub fn data_health_undo_status(&self) -> AppResult<DataHealthUndoStatus> {
        let connection = self.database.lock()?;
        let row = connection
            .query_row(
                "SELECT backup_path, created_at_ms FROM data_health_undo WHERE singleton_id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((backup_path, created_at_ms)) => DataHealthUndoStatus {
                available: true,
                backup_path: Some(backup_path),
                created_at_ms: Some(created_at_ms),
            },
            None => DataHealthUndoStatus {
                available: false,
                backup_path: None,
                created_at_ms: None,
            },
        })
    }

    pub fn undo_data_health_repair(&self) -> AppResult<usize> {
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let snapshot_json: String = transaction
            .query_row(
                "SELECT snapshot_json FROM data_health_undo WHERE singleton_id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::InvalidSession("no health repair can be undone".to_owned()))?;
        let snapshot: DataHealthUndoSnapshot =
            serde_json::from_str(&snapshot_json).map_err(|error| {
                AppError::InvalidSession(format!("invalid health repair snapshot: {error}"))
            })?;
        for session in &snapshot.sessions {
            transaction.execute("DELETE FROM activity_sessions WHERE id = ?1", [session.id])?;
            transaction.execute(
                "INSERT INTO activity_sessions (
                   id, state, application_id, window_title, started_at_ms, ended_at_ms,
                   duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms, note
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    session.id,
                    session.state.as_db_str(),
                    session.application_id,
                    session.window_title,
                    session.started_at_ms,
                    session.ended_at_ms,
                    session.duration_ms,
                    session.is_open,
                    session.closed_reason.map(ClosedReason::as_db_str),
                    session.created_at_ms,
                    session.updated_at_ms,
                    session.note,
                ],
            )?;
        }
        transaction.execute("DELETE FROM data_health_undo WHERE singleton_id = 1", [])?;
        transaction.commit()?;
        Ok(snapshot.sessions.len())
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
        if !(0..=1440).contains(&settings.daily_focus_goal_minutes) {
            return Err(AppError::InvalidMonitorConfiguration(
                "daily focus goal must be between 0 and 1440 minutes".to_owned(),
            ));
        }
        if !(1..=60).contains(&settings.focus_block_gap_minutes) {
            return Err(AppError::InvalidMonitorConfiguration(
                "focus block gap must be between 1 and 60 minutes".to_owned(),
            ));
        }
        if !matches!(settings.break_reminder_minutes, 30 | 45 | 60 | 90 | 120) {
            return Err(AppError::InvalidMonitorConfiguration(
                "break reminder must be 30, 45, 60, 90, or 120 minutes".to_owned(),
            ));
        }
        validate_clock_time(&settings.quiet_hours_start)?;
        validate_clock_time(&settings.quiet_hours_end)?;
        let connection = self.database.lock()?;
        connection.execute(
            "UPDATE settings SET idle_threshold_seconds = ?1, launch_at_login = ?2,
                start_tracking_automatically = ?3, hide_to_tray_on_close = ?4,
                record_window_titles = ?5, appearance = ?6,
                retention_days = ?7, automatic_backup_enabled = ?8,
                backup_interval = ?9, backup_keep_count = ?10,
                backup_directory = ?11, daily_focus_goal_minutes = ?12,
                focus_block_gap_minutes = ?13, break_reminders_enabled = ?14,
                break_reminder_minutes = ?15, quiet_hours_start = ?16,
                quiet_hours_end = ?17, updated_at_ms = ?18
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
                settings.daily_focus_goal_minutes,
                settings.focus_block_gap_minutes,
                settings.break_reminders_enabled,
                settings.break_reminder_minutes,
                settings.quiet_hours_start,
                settings.quiet_hours_end,
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

    pub fn update_session_notes(
        &self,
        session_ids: &[i64],
        note: Option<&str>,
    ) -> AppResult<usize> {
        if session_ids.is_empty() {
            return Ok(0);
        }
        let note = note.map(str::trim).filter(|value| !value.is_empty());
        if note.is_some_and(|value| value.chars().count() > 500) {
            return Err(AppError::InvalidSession(
                "session notes cannot exceed 500 characters".to_owned(),
            ));
        }
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let mut affected = 0;
        for id in session_ids {
            affected += transaction.execute(
                "UPDATE activity_sessions SET note = ?2, updated_at_ms = MAX(updated_at_ms, ?3)
                 WHERE id = ?1 AND is_open = 0",
                params![id, note, now_millis()],
            )?;
        }
        transaction.commit()?;
        Ok(affected)
    }

    pub fn update_session_application_categories(
        &self,
        session_ids: &[i64],
        category: &str,
    ) -> AppResult<usize> {
        let category = category.trim();
        if category.is_empty() || category.chars().count() > 40 {
            return Err(AppError::InvalidSession(
                "application category must contain between 1 and 40 characters".to_owned(),
            ));
        }
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let mut affected = 0;
        for id in session_ids {
            affected += transaction.execute(
                "UPDATE applications SET category = ?2 WHERE id = (
                    SELECT application_id FROM activity_sessions WHERE id = ?1
                 )",
                params![id, category],
            )?;
        }
        transaction.commit()?;
        Ok(affected)
    }

    pub fn delete_closed_sessions(&self, session_ids: &[i64]) -> AppResult<TimelineMutationResult> {
        self.destructive_session_edit(session_ids, "Deleted sessions", |transaction, ids| {
            let mut affected = 0;
            for id in ids {
                affected += transaction.execute(
                    "DELETE FROM activity_sessions WHERE id = ?1 AND is_open = 0",
                    [id],
                )?;
            }
            Ok(affected)
        })
    }

    pub fn merge_closed_sessions(&self, session_ids: &[i64]) -> AppResult<TimelineMutationResult> {
        self.destructive_session_edit(session_ids, "Merged sessions", |transaction, ids| {
            if ids.len() < 2 {
                return Err(AppError::InvalidSession(
                    "select at least two sessions to merge".to_owned(),
                ));
            }
            let mut sessions = Vec::with_capacity(ids.len());
            for id in ids {
                let session =
                    find_session(transaction, *id)?.ok_or(AppError::SessionNotFound(*id))?;
                if session.is_open {
                    return Err(AppError::InvalidSession(
                        "open sessions cannot be merged".to_owned(),
                    ));
                }
                sessions.push(session);
            }
            sessions.sort_by_key(|session| (session.started_at_ms, session.id));
            let first = &sessions[0];
            for pair in sessions.windows(2) {
                if pair[0].state != pair[1].state
                    || pair[0].application_id != pair[1].application_id
                    || pair[0].window_title != pair[1].window_title
                {
                    return Err(AppError::InvalidSession(
                        "only compatible sessions can be merged".to_owned(),
                    ));
                }
                let intervening: bool = transaction.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM activity_sessions
                        WHERE id NOT IN (?1, ?2)
                          AND started_at_ms < ?4 AND ended_at_ms > ?3
                    )",
                    params![
                        pair[0].id,
                        pair[1].id,
                        pair[0].ended_at_ms,
                        pair[1].started_at_ms
                    ],
                    |row| row.get(0),
                )?;
                if intervening {
                    return Err(AppError::InvalidSession(
                        "selected sessions are not adjacent".to_owned(),
                    ));
                }
            }
            let end = sessions
                .iter()
                .map(|session| session.ended_at_ms)
                .max()
                .unwrap();
            let notes = sessions
                .iter()
                .filter_map(|session| session.note.as_deref())
                .filter(|note| !note.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            transaction.execute(
                "UPDATE activity_sessions SET ended_at_ms = ?2, duration_ms = ?2 - started_at_ms,
                 note = NULLIF(?3, ''), updated_at_ms = ?2 WHERE id = ?1",
                params![first.id, end, notes],
            )?;
            for session in sessions.iter().skip(1) {
                transaction.execute("DELETE FROM activity_sessions WHERE id = ?1", [session.id])?;
            }
            Ok(sessions.len())
        })
    }

    pub fn split_closed_session(
        &self,
        session_id: i64,
        split_at_ms: i64,
    ) -> AppResult<TimelineMutationResult> {
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let original =
            find_session(&transaction, session_id)?.ok_or(AppError::SessionNotFound(session_id))?;
        if original.is_open {
            return Err(AppError::InvalidSession(
                "open sessions cannot be split".to_owned(),
            ));
        }
        if split_at_ms <= original.started_at_ms || split_at_ms >= original.ended_at_ms {
            return Err(AppError::InvalidSession(
                "split time must be inside the session".to_owned(),
            ));
        }
        transaction.execute(
            "UPDATE activity_sessions
             SET ended_at_ms = ?2, duration_ms = ?2 - started_at_ms, updated_at_ms = ?2
             WHERE id = ?1",
            params![session_id, split_at_ms],
        )?;
        transaction.execute(
            "INSERT INTO activity_sessions (
                state, application_id, window_title, started_at_ms, ended_at_ms,
                duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms, note
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5 - ?4, 0, ?6, ?7, ?8, ?9)",
            params![
                original.state.as_db_str(),
                original.application_id,
                original.window_title.as_deref(),
                split_at_ms,
                original.ended_at_ms,
                original.closed_reason.map(ClosedReason::as_db_str),
                original.created_at_ms,
                original.updated_at_ms,
                original.note.as_deref(),
            ],
        )?;
        let new_id = transaction.last_insert_rowid();
        let token = format!("{}-{session_id}", now_millis());
        let snapshot = TimelineUndoSnapshot {
            sessions: vec![original],
            delete_session_ids: vec![session_id, new_id],
            operation_label: Some("Split session".to_owned()),
        };
        transaction.execute(
            "INSERT INTO timeline_undo(token, snapshot_json, created_at_ms) VALUES (?1, ?2, ?3)",
            params![
                token,
                serde_json::to_string(&snapshot).map_err(|error| {
                    AppError::InvalidSession(format!("could not create undo snapshot: {error}"))
                })?,
                now_millis()
            ],
        )?;
        transaction.commit()?;
        Ok(TimelineMutationResult {
            affected_count: 2,
            undo_token: Some(token),
        })
    }

    fn destructive_session_edit<F>(
        &self,
        session_ids: &[i64],
        operation_label: &str,
        operation: F,
    ) -> AppResult<TimelineMutationResult>
    where
        F: FnOnce(&rusqlite::Transaction<'_>, &[i64]) -> AppResult<usize>,
    {
        if session_ids.is_empty() {
            return Ok(TimelineMutationResult {
                affected_count: 0,
                undo_token: None,
            });
        }
        let mut ids = session_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let mut snapshot = Vec::with_capacity(ids.len());
        for id in &ids {
            let session = find_session(&transaction, *id)?.ok_or(AppError::SessionNotFound(*id))?;
            if session.is_open {
                return Err(AppError::InvalidSession(
                    "open sessions cannot be changed".to_owned(),
                ));
            }
            snapshot.push(session);
        }
        let affected_count = operation(&transaction, &ids)?;
        let token = format!("{}-{}", now_millis(), snapshot[0].id);
        transaction.execute(
            "DELETE FROM timeline_undo WHERE created_at_ms < ?1",
            [now_millis().saturating_sub(24 * 60 * 60 * 1_000)],
        )?;
        transaction.execute(
            "INSERT INTO timeline_undo(token, snapshot_json, created_at_ms) VALUES (?1, ?2, ?3)",
            params![
                token,
                serde_json::to_string(&TimelineUndoSnapshot {
                    sessions: snapshot,
                    delete_session_ids: ids.clone(),
                    operation_label: Some(operation_label.to_owned()),
                })
                .map_err(|error| {
                    AppError::InvalidSession(format!("could not create undo snapshot: {error}"))
                })?,
                now_millis()
            ],
        )?;
        transaction.commit()?;
        Ok(TimelineMutationResult {
            affected_count,
            undo_token: Some(token),
        })
    }

    pub fn undo_timeline_edit(&self, token: &str) -> AppResult<usize> {
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let snapshot_json: String = transaction
            .query_row(
                "SELECT snapshot_json FROM timeline_undo WHERE token = ?1",
                [token],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::InvalidSession("undo is no longer available".to_owned()))?;
        let snapshot: TimelineUndoSnapshot = serde_json::from_str(&snapshot_json)
            .or_else(|_| {
                serde_json::from_str::<Vec<ActivitySession>>(&snapshot_json).map(|sessions| {
                    TimelineUndoSnapshot {
                        delete_session_ids: sessions.iter().map(|session| session.id).collect(),
                        sessions,
                        operation_label: None,
                    }
                })
            })
            .map_err(|error| AppError::InvalidSession(format!("invalid undo snapshot: {error}")))?;
        for session_id in &snapshot.delete_session_ids {
            transaction.execute("DELETE FROM activity_sessions WHERE id = ?1", [session_id])?;
        }
        for session in &snapshot.sessions {
            transaction.execute(
                "INSERT INTO activity_sessions (
                    id, state, application_id, window_title, started_at_ms, ended_at_ms,
                    duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms, note
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    session.id,
                    session.state.as_db_str(),
                    session.application_id,
                    session.window_title,
                    session.started_at_ms,
                    session.ended_at_ms,
                    session.duration_ms,
                    session.is_open,
                    session.closed_reason.map(ClosedReason::as_db_str),
                    session.created_at_ms,
                    session.updated_at_ms,
                    session.note,
                ],
            )?;
        }
        transaction.execute("DELETE FROM timeline_undo WHERE token = ?1", [token])?;
        transaction.commit()?;
        Ok(snapshot.sessions.len())
    }

    pub fn timeline_undo_tokens(&self) -> AppResult<Vec<String>> {
        Ok(self
            .timeline_undo_history()?
            .into_iter()
            .map(|entry| entry.token)
            .collect())
    }

    pub fn timeline_undo_history(&self) -> AppResult<Vec<TimelineUndoEntry>> {
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT token, snapshot_json, created_at_ms FROM timeline_undo
             WHERE created_at_ms >= ?1
             ORDER BY created_at_ms, token",
        )?;
        let snapshots = statement
            .query_map([now_millis().saturating_sub(24 * 60 * 60 * 1_000)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        snapshots
            .into_iter()
            .map(|(token, snapshot_json, created_at_ms)| {
                let snapshot = serde_json::from_str::<TimelineUndoSnapshot>(&snapshot_json)
                    .or_else(|_| {
                        serde_json::from_str::<Vec<ActivitySession>>(&snapshot_json).map(
                            |sessions| TimelineUndoSnapshot {
                                delete_session_ids: sessions
                                    .iter()
                                    .map(|session| session.id)
                                    .collect(),
                                sessions,
                                operation_label: None,
                            },
                        )
                    })
                    .map_err(|error| {
                        AppError::InvalidSession(format!("invalid undo snapshot: {error}"))
                    })?;
                Ok(TimelineUndoEntry {
                    token,
                    created_at_ms,
                    session_count: snapshot.sessions.len(),
                    operation_label: snapshot
                        .operation_label
                        .unwrap_or_else(|| "Timeline edit".to_owned()),
                })
            })
            .collect()
    }

    pub fn import_conflict_count(&self, records: &[ImportRecord]) -> AppResult<usize> {
        let connection = self.database.lock()?;
        let mut conflicts = 0;
        for record in records {
            validate_import_record(record)?;
            let exists: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM activity_sessions
                    WHERE started_at_ms < ?2 AND ended_at_ms > ?1
                )",
                params![record.started_at_ms, record.ended_at_ms],
                |row| row.get(0),
            )?;
            conflicts += usize::from(exists);
        }
        Ok(conflicts)
    }

    pub fn import_records(
        &self,
        records: &[ImportRecord],
        merge_conflicts: bool,
    ) -> AppResult<(usize, usize, usize)> {
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let mut imported = 0;
        let mut merged = 0;
        let mut skipped = 0;
        for record in records {
            validate_import_record(record)?;
            let application_id = if record.state == ActivityState::Active {
                let name = record
                    .application_name
                    .as_deref()
                    .unwrap_or("Imported application");
                let identity_key = record
                    .bundle_identifier
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(|bundle| format!("bundle:{bundle}"))
                    .unwrap_or_else(|| format!("name:{name}"));
                transaction.execute(
                    "INSERT INTO applications (
                        identity_key, name, bundle_id, executable_path,
                        first_seen_at_ms, last_seen_at_ms
                     ) VALUES (?1, ?2, ?3, NULL, ?4, ?5)
                     ON CONFLICT(identity_key) DO UPDATE SET
                        name = excluded.name,
                        last_seen_at_ms = MAX(applications.last_seen_at_ms, excluded.last_seen_at_ms)",
                    params![
                        identity_key, name, record.bundle_identifier,
                        record.started_at_ms, record.ended_at_ms
                    ],
                )?;
                Some(transaction.query_row(
                    "SELECT id FROM applications WHERE identity_key = ?1",
                    [identity_key],
                    |row| row.get(0),
                )?)
            } else {
                None
            };
            let overlapping: Option<OverlappingSession> = transaction
                .query_row(
                    "SELECT id, state, application_id, window_title,
                                started_at_ms, ended_at_ms
                         FROM activity_sessions
                         WHERE started_at_ms < ?2 AND ended_at_ms > ?1
                         ORDER BY started_at_ms LIMIT 1",
                    params![record.started_at_ms, record.ended_at_ms],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((id, state, existing_app, title, start, end)) = overlapping {
                let compatible = state == record.state.as_db_str()
                    && existing_app == application_id
                    && title == record.window_title;
                if merge_conflicts && compatible {
                    let merged_start = start.min(record.started_at_ms);
                    let merged_end = end.max(record.ended_at_ms);
                    transaction.execute(
                        "UPDATE activity_sessions SET started_at_ms = ?2, ended_at_ms = ?3,
                         duration_ms = ?3 - ?2, note = COALESCE(note, ?4), updated_at_ms = ?3
                         WHERE id = ?1 AND is_open = 0",
                        params![id, merged_start, merged_end, record.note],
                    )?;
                    merged += 1;
                } else {
                    skipped += 1;
                }
                continue;
            }
            transaction.execute(
                "INSERT INTO activity_sessions (
                    state, application_id, window_title, started_at_ms, ended_at_ms,
                    duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms, note
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?5 - ?4, 0, 'SHUTDOWN', ?4, ?5, ?6)",
                params![
                    record.state.as_db_str(),
                    application_id,
                    record.window_title,
                    record.started_at_ms,
                    record.ended_at_ms,
                    record.note
                ],
            )?;
            imported += 1;
        }
        transaction.commit()?;
        Ok((imported, merged, skipped))
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
                s.closed_reason, s.created_at_ms, s.updated_at_ms, s.note,
                a.id, a.identity_key, a.name, a.bundle_id, a.executable_path,
                a.category, a.is_ignored, a.record_window_titles,
                a.first_seen_at_ms, a.last_seen_at_ms
             FROM activity_sessions s
             LEFT JOIN applications a ON a.id = s.application_id
             WHERE s.started_at_ms < ?2 AND s.ended_at_ms > ?1
             ORDER BY s.started_at_ms, s.id",
        )?;
        let records = statement
            .query_map(params![range_start_ms, range_end_ms], |row| {
                let session = map_session(row)?;
                let application_id: Option<i64> = row.get(12)?;
                let application = application_id
                    .map(|id| {
                        Ok::<Application, rusqlite::Error>(Application {
                            id,
                            identity_key: row.get(13)?,
                            name: row.get(14)?,
                            bundle_id: row.get(15)?,
                            executable_path: row.get(16)?,
                            category: row.get(17)?,
                            is_ignored: row.get(18)?,
                            record_window_titles: row.get(19)?,
                            first_seen_at_ms: row.get(20)?,
                            last_seen_at_ms: row.get(21)?,
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

    pub fn records_overlapping_page(
        &self,
        range_start_ms: i64,
        range_end_ms: i64,
        offset: usize,
        limit: usize,
    ) -> AppResult<Vec<ActivityRecord>> {
        self.records_overlapping_page_filtered(
            range_start_ms,
            range_end_ms,
            offset,
            limit,
            &TimelineSearch::default(),
        )
    }

    pub fn records_overlapping_page_filtered(
        &self,
        range_start_ms: i64,
        range_end_ms: i64,
        offset: usize,
        limit: usize,
        search: &TimelineSearch,
    ) -> AppResult<Vec<ActivityRecord>> {
        if range_end_ms <= range_start_ms || !(1..=1_000).contains(&limit) {
            return Err(AppError::InvalidTimeRange(
                "invalid timeline page range or limit".to_owned(),
            ));
        }
        let parameters = TimelineSearchParameters::new(search)?;
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(&format!(
            "SELECT
                s.id, s.state, s.application_id, s.window_title,
                s.started_at_ms, s.ended_at_ms, s.duration_ms, s.is_open,
                s.closed_reason, s.created_at_ms, s.updated_at_ms, s.note,
                a.id, a.identity_key, a.name, a.bundle_id, a.executable_path,
                a.category, a.is_ignored, a.record_window_titles,
                a.first_seen_at_ms, a.last_seen_at_ms
             FROM activity_sessions s
             LEFT JOIN applications a ON a.id = s.application_id
             WHERE {TIMELINE_SEARCH_WHERE}
             ORDER BY s.started_at_ms, s.id
             LIMIT ?9 OFFSET ?10"
        ))?;
        Ok(statement
            .query_map(
                params![
                    range_start_ms,
                    range_end_ms,
                    parameters.state,
                    parameters.minimum_duration_ms,
                    parameters.maximum_duration_ms,
                    parameters.time_from_minutes,
                    parameters.time_to_minutes,
                    parameters.query_pattern,
                    limit as i64,
                    offset as i64,
                ],
                map_activity_record,
            )?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn timeline_page_totals(
        &self,
        range_start_ms: i64,
        range_end_ms: i64,
    ) -> AppResult<(usize, i64, i64)> {
        self.timeline_page_totals_filtered(range_start_ms, range_end_ms, &TimelineSearch::default())
    }

    pub fn timeline_page_totals_filtered(
        &self,
        range_start_ms: i64,
        range_end_ms: i64,
        search: &TimelineSearch,
    ) -> AppResult<(usize, i64, i64)> {
        if range_end_ms <= range_start_ms {
            return Err(AppError::InvalidTimeRange(
                "timeline range end must be after its start".to_owned(),
            ));
        }
        let parameters = TimelineSearchParameters::new(search)?;
        let connection = self.database.lock()?;
        connection
            .query_row(
                &format!(
                    "SELECT COUNT(*),
                   COALESCE(SUM(CASE WHEN s.state = 'ACTIVE'
                     THEN MIN(s.ended_at_ms, ?2) - MAX(s.started_at_ms, ?1) ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN s.state = 'IDLE'
                     THEN MIN(s.ended_at_ms, ?2) - MAX(s.started_at_ms, ?1) ELSE 0 END), 0)
                 FROM activity_sessions s
                 LEFT JOIN applications a ON a.id = s.application_id
                 WHERE {TIMELINE_SEARCH_WHERE}"
                ),
                params![
                    range_start_ms,
                    range_end_ms,
                    parameters.state,
                    parameters.minimum_duration_ms,
                    parameters.maximum_duration_ms,
                    parameters.time_from_minutes,
                    parameters.time_to_minutes,
                    parameters.query_pattern,
                ],
                |row| Ok((row.get::<_, i64>(0)? as usize, row.get(1)?, row.get(2)?)),
            )
            .map_err(Into::into)
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

const TIMELINE_SEARCH_WHERE: &str = r#"
    s.started_at_ms < ?2 AND s.ended_at_ms > ?1
    AND (?3 IS NULL OR s.state = ?3)
    AND (
      ?4 IS NULL
      OR MIN(s.ended_at_ms, ?2) - MAX(s.started_at_ms, ?1) >= ?4
    )
    AND (
      ?5 IS NULL
      OR MIN(s.ended_at_ms, ?2) - MAX(s.started_at_ms, ?1) <= ?5
    )
    AND (
      ?6 IS NULL
      OR (
        CAST(strftime(
          '%H', MAX(s.started_at_ms, ?1) / 1000, 'unixepoch', 'localtime'
        ) AS INTEGER) * 60
        + CAST(strftime(
          '%M', MAX(s.started_at_ms, ?1) / 1000, 'unixepoch', 'localtime'
        ) AS INTEGER)
      ) >= ?6
    )
    AND (
      ?7 IS NULL
      OR (
        CAST(strftime(
          '%H', MAX(s.started_at_ms, ?1) / 1000, 'unixepoch', 'localtime'
        ) AS INTEGER) * 60
        + CAST(strftime(
          '%M', MAX(s.started_at_ms, ?1) / 1000, 'unixepoch', 'localtime'
        ) AS INTEGER)
      ) <= ?7
    )
    AND (
      ?8 IS NULL
      OR COALESCE(a.name, '') LIKE ?8 ESCAPE '\'
      OR COALESCE(a.bundle_id, '') LIKE ?8 ESCAPE '\'
      OR COALESCE(a.category, '') LIKE ?8 ESCAPE '\'
      OR COALESCE(s.window_title, '') LIKE ?8 ESCAPE '\'
      OR COALESCE(s.note, '') LIKE ?8 ESCAPE '\'
    )
"#;

struct TimelineSearchParameters {
    state: Option<&'static str>,
    minimum_duration_ms: Option<i64>,
    maximum_duration_ms: Option<i64>,
    time_from_minutes: Option<i64>,
    time_to_minutes: Option<i64>,
    query_pattern: Option<String>,
}

impl TimelineSearchParameters {
    fn new(search: &TimelineSearch) -> AppResult<Self> {
        if search
            .query
            .as_deref()
            .is_some_and(|query| query.chars().count() > 200)
        {
            return Err(AppError::InvalidTimeRange(
                "timeline search query cannot exceed 200 characters".to_owned(),
            ));
        }
        if [search.minimum_duration_ms, search.maximum_duration_ms]
            .into_iter()
            .flatten()
            .any(|duration| duration < 0)
        {
            return Err(AppError::InvalidTimeRange(
                "timeline duration filters cannot be negative".to_owned(),
            ));
        }
        if [search.time_from_minutes, search.time_to_minutes]
            .into_iter()
            .flatten()
            .any(|minutes| !(0..24 * 60).contains(&minutes))
        {
            return Err(AppError::InvalidTimeRange(
                "timeline time filters must be valid local clock minutes".to_owned(),
            ));
        }

        let query_pattern = search
            .query
            .as_deref()
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(escape_like_pattern);
        Ok(Self {
            state: search.state.map(ActivityState::as_db_str),
            minimum_duration_ms: search.minimum_duration_ms,
            maximum_duration_ms: search.maximum_duration_ms,
            time_from_minutes: search.time_from_minutes,
            time_to_minutes: search.time_to_minutes,
            query_pattern,
        })
    }
}

fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('%');
    for character in value.chars() {
        if matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped.push('%');
    escaped
}

const SESSION_SELECT: &str = "SELECT
    id, state, application_id, window_title, started_at_ms, ended_at_ms,
    duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms, note
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

fn validate_import_record(record: &ImportRecord) -> AppResult<()> {
    if record.ended_at_ms <= record.started_at_ms {
        return Err(AppError::InvalidTimeRange(
            "imported session end must be after its start".to_owned(),
        ));
    }
    if record.state == ActivityState::Active
        && record
            .application_name
            .as_deref()
            .is_none_or(|name| name.trim().is_empty())
    {
        return Err(AppError::InvalidSession(
            "imported active sessions require an application name".to_owned(),
        ));
    }
    if record
        .note
        .as_ref()
        .is_some_and(|note| note.chars().count() > 500)
    {
        return Err(AppError::InvalidSession(
            "imported session notes cannot exceed 500 characters".to_owned(),
        ));
    }
    Ok(())
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn retention_cutoff(now_ms: i64, retention_days: i64) -> Option<i64> {
    if retention_days == 0 {
        None
    } else {
        Some(now_ms.saturating_sub(retention_days.saturating_mul(24 * 60 * 60 * 1_000)))
    }
}

fn validate_clock_time(value: &str) -> AppResult<()> {
    let valid = value.len() == 5
        && value.as_bytes().get(2) == Some(&b':')
        && value[..2].parse::<u8>().is_ok_and(|hour| hour < 24)
        && value[3..].parse::<u8>().is_ok_and(|minute| minute < 60);
    if valid {
        Ok(())
    } else {
        Err(AppError::InvalidMonitorConfiguration(
            "quiet hours must use HH:MM in 24-hour time".to_owned(),
        ))
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
        record_window_titles: row.get(7)?,
        first_seen_at_ms: row.get(8)?,
        last_seen_at_ms: row.get(9)?,
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
        note: row.get(11)?,
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

fn map_activity_record(row: &Row<'_>) -> rusqlite::Result<ActivityRecord> {
    let session = map_session(row)?;
    let application_id: Option<i64> = row.get(12)?;
    let application = application_id
        .map(|id| {
            Ok::<Application, rusqlite::Error>(Application {
                id,
                identity_key: row.get(13)?,
                name: row.get(14)?,
                bundle_id: row.get(15)?,
                executable_path: row.get(16)?,
                category: row.get(17)?,
                is_ignored: row.get(18)?,
                record_window_titles: row.get(19)?,
                first_seen_at_ms: row.get(20)?,
                last_seen_at_ms: row.get(21)?,
            })
        })
        .transpose()?;
    Ok(ActivityRecord {
        session,
        application,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

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
    fn shortcut_settings_are_persisted_and_can_be_disabled() {
        let repository = repository();
        let defaults = repository.shortcut_settings().unwrap();
        assert_eq!(
            defaults.toggle_focus.as_deref(),
            Some("CommandOrControl+Shift+F")
        );
        let updated = repository
            .update_shortcut_settings(
                &ShortcutSettings {
                    toggle_focus: Some("CommandOrControl+Shift+2".to_owned()),
                    pause_focus: None,
                    start_template: Some("CommandOrControl+Shift+3".to_owned()),
                },
                123,
            )
            .unwrap();
        assert_eq!(updated.pause_focus, None);
        assert_eq!(
            repository.shortcut_settings().unwrap().toggle_focus,
            Some("CommandOrControl+Shift+2".to_owned())
        );
    }

    #[test]
    fn focus_mode_state_persists_until_explicitly_ended() {
        let repository = repository();
        assert_eq!(
            repository.focus_mode_status().expect("status should load"),
            PersistedFocusMode {
                active: false,
                started_at_ms: None,
                planned_end_at_ms: None,
                paused: false,
                paused_at_ms: None,
                total_paused_ms: 0,
                template_id: None,
            }
        );

        repository
            .update_focus_mode(
                &PersistedFocusMode {
                    active: true,
                    started_at_ms: Some(1_234),
                    planned_end_at_ms: Some(61_234),
                    paused: false,
                    paused_at_ms: None,
                    total_paused_ms: 0,
                    template_id: None,
                },
                1_234,
            )
            .expect("focus mode should start");
        let active = repository
            .focus_mode_status()
            .expect("status should reload");
        assert!(active.active);
        assert_eq!(active.started_at_ms, Some(1_234));
        assert_eq!(active.planned_end_at_ms, Some(61_234));

        repository
            .update_focus_mode(
                &PersistedFocusMode {
                    active: false,
                    started_at_ms: None,
                    planned_end_at_ms: None,
                    paused: false,
                    paused_at_ms: None,
                    total_paused_ms: 0,
                    template_id: None,
                },
                2_345,
            )
            .expect("focus mode should end");
        assert!(
            !repository
                .focus_mode_status()
                .expect("status should reload")
                .active
        );
    }

    #[test]
    fn focus_plan_history_filters_range_and_orders_recent_first() {
        let repository = repository();
        repository
            .record_focus_plan_outcome(1_000, Some(61_000), 51_000, 5_000, true, None)
            .expect("completed plan should be stored");
        repository
            .record_focus_plan_outcome(70_000, Some(130_000), 90_000, 0, false, None)
            .expect("cancelled plan should be stored");

        let entries = repository
            .focus_plan_history(50_000, 100_000)
            .expect("history should load");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].outcome, "CANCELLED");
        assert_eq!(entries[1].outcome, "COMPLETED");
        assert_eq!(
            repository
                .focus_plan_history(80_000, 100_000)
                .expect("filtered history should load")
                .len(),
            1
        );
    }

    #[test]
    fn focus_plan_templates_can_be_created_and_deleted() {
        let repository = repository();
        assert_eq!(repository.focus_plan_templates().unwrap().len(), 2);
        let template = repository
            .create_focus_plan_template("Writing", 75)
            .unwrap();
        assert_eq!(template.name, "Writing");
        assert_eq!(template.duration_minutes, 75);
        repository
            .mark_focus_template_started(template.id, 100)
            .unwrap();
        repository
            .record_focus_plan_outcome(100, Some(4_500_100), 4_500_100, 0, true, Some(template.id))
            .unwrap();
        let updated = repository
            .update_focus_plan_template(template.id, "Long writing", 80, -1)
            .unwrap();
        assert_eq!(updated.use_count, 1);
        assert_eq!(updated.completed_count, 1);
        assert_eq!(updated.sort_order, -1);
        repository.delete_focus_plan_template(template.id).unwrap();
        assert_eq!(repository.focus_plan_templates().unwrap().len(), 2);
        assert!(repository.create_focus_plan_template("", 4).is_err());
    }

    #[test]
    fn data_health_repair_trims_overlaps_and_removes_zero_duration_sessions() {
        let repository = repository();
        {
            let connection = repository.database.lock().unwrap();
            for (start, end) in [(100, 300), (200, 400), (500, 500)] {
                connection
                    .execute(
                        "INSERT INTO activity_sessions (
                           state, application_id, started_at_ms, ended_at_ms, duration_ms,
                           is_open, closed_reason, created_at_ms, updated_at_ms
                         ) VALUES ('IDLE', NULL, ?1, ?2, ?2 - ?1, 0, 'BECAME_ACTIVE', ?1, ?2)",
                        params![start, end],
                    )
                    .unwrap();
            }
        }
        let summary = repository.data_health_summary().unwrap();
        assert!(summary.overlapping_session_count >= 2);
        assert_eq!(summary.zero_duration_session_count, 1);

        let repaired = repository
            .repair_data_health("/tmp/watchhouse-test-backup.sqlite3")
            .unwrap();
        assert_eq!(repaired.trimmed_session_count, 1);
        assert_eq!(repaired.deleted_session_count, 1);
        assert_eq!(repaired.backup_path, "/tmp/watchhouse-test-backup.sqlite3");
        assert!(repository.data_health_undo_status().unwrap().available);
        let sessions = repository.records_overlapping(0, 600).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session.ended_at_ms, 200);
        assert_eq!(
            repository.data_health_summary().unwrap(),
            DataHealthSummary {
                overlapping_session_count: 0,
                zero_duration_session_count: 0,
            }
        );
        assert_eq!(repository.undo_data_health_repair().unwrap(), 3);
        assert_eq!(repository.records_overlapping(0, 600).unwrap().len(), 3);
        assert!(!repository.data_health_undo_status().unwrap().available);
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
            .update_application_preferences(app.id, "Work", true, true)
            .expect("preferences should update");
        assert_eq!(preferences.category, "Work");
        assert!(preferences.is_ignored);
        assert!(preferences.record_window_titles);
        assert!(
            !repository
                .should_record_window_title(&preferences.identity_key)
                .expect("global title setting should load")
        );
        let mut settings = repository.settings().expect("settings should load");
        settings.record_window_titles = true;
        repository
            .update_settings(&settings, 101)
            .expect("global title setting should update");
        assert!(
            repository
                .should_record_window_title(&preferences.identity_key)
                .expect("combined title policy should load")
        );

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
            .update_application_preferences(ignored.id, "Personal", true, false)
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
        assert_eq!(
            repository.timeline_page_totals(0, 300).unwrap(),
            (3, 300, 0)
        );
        let first_page = repository.records_overlapping_page(0, 300, 0, 2).unwrap();
        let second_page = repository.records_overlapping_page(0, 300, 2, 2).unwrap();
        assert_eq!(first_page.len(), 2);
        assert_eq!(second_page.len(), 1);
        assert!(first_page[1].session.id < second_page[0].session.id);
    }

    #[test]
    fn timeline_search_filters_and_totals_are_applied_in_sql() {
        let repository = repository();
        let idea = application(&repository, 0);
        repository
            .update_application_preferences(idea.id, "Work", false, false)
            .expect("application category should update");
        let terminal = repository
            .upsert_application(&NewApplication {
                name: "Terminal".to_owned(),
                bundle_id: Some("example.terminal".to_owned()),
                executable_path: None,
                seen_at_ms: 0,
            })
            .expect("second application should be stored");
        let local_timestamp = |hour: u32, minute: u32| {
            Local
                .with_ymd_and_hms(2025, 1, 15, hour, minute, 0)
                .single()
                .expect("test date should resolve locally")
                .timestamp_millis()
        };
        let range_start = local_timestamp(0, 0);
        let range_end = Local
            .with_ymd_and_hms(2025, 1, 16, 0, 0, 0)
            .single()
            .expect("next test date should resolve locally")
            .timestamp_millis();

        let first = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(idea.id),
                window_title: Some("Project Alpha".to_owned()),
                started_at_ms: local_timestamp(9, 0),
            })
            .expect("first session should open");
        repository
            .close_session(first.id, local_timestamp(9, 30), ClosedReason::AppChanged)
            .expect("first session should close");
        repository
            .update_session_notes(&[first.id], Some("Sprint 100% review"))
            .expect("session note should update");

        let idle = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                started_at_ms: local_timestamp(9, 30),
            })
            .expect("idle session should open");
        repository
            .close_session(idle.id, local_timestamp(9, 40), ClosedReason::BecameActive)
            .expect("idle session should close");

        let second = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(terminal.id),
                window_title: Some("Shell".to_owned()),
                started_at_ms: local_timestamp(10, 0),
            })
            .expect("second session should open");
        repository
            .close_session(second.id, local_timestamp(11, 20), ClosedReason::AppChanged)
            .expect("second session should close");

        for query in ["alpha", "INTELLIJ", "work", "%"] {
            let search = TimelineSearch {
                query: Some(query.to_owned()),
                ..TimelineSearch::default()
            };
            assert_eq!(
                repository
                    .timeline_page_totals_filtered(range_start, range_end, &search)
                    .expect("search totals should succeed"),
                (1, 30 * 60_000, 0),
                "query {query:?} should match only the first session",
            );
        }

        let idle_only = TimelineSearch {
            state: Some(ActivityState::Idle),
            ..TimelineSearch::default()
        };
        assert_eq!(
            repository
                .timeline_page_totals_filtered(range_start, range_end, &idle_only)
                .expect("state totals should succeed"),
            (1, 0, 10 * 60_000),
        );

        let long_sessions = TimelineSearch {
            minimum_duration_ms: Some(45 * 60_000),
            ..TimelineSearch::default()
        };
        assert_eq!(
            repository
                .records_overlapping_page_filtered(range_start, range_end, 0, 10, &long_sessions,)
                .expect("duration search should succeed")[0]
                .session
                .id,
            second.id,
        );

        let morning_end = TimelineSearch {
            time_to_minutes: Some(9 * 60 + 30),
            ..TimelineSearch::default()
        };
        assert_eq!(
            repository
                .timeline_page_totals_filtered(range_start, range_end, &morning_end)
                .expect("end time totals should succeed")
                .0,
            2,
        );
        let from_ten = TimelineSearch {
            time_from_minutes: Some(10 * 60),
            ..TimelineSearch::default()
        };
        assert_eq!(
            repository
                .timeline_page_totals_filtered(range_start, range_end, &from_ten)
                .expect("start time totals should succeed"),
            (1, 80 * 60_000, 0),
        );
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

    #[test]
    fn destructive_batch_edit_can_be_undone() {
        let repository = repository();
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                started_at_ms: 100,
            })
            .unwrap();
        repository
            .close_session(session.id, 200, ClosedReason::BecameActive)
            .unwrap();
        let result = repository.delete_closed_sessions(&[session.id]).unwrap();
        assert!(repository.records_overlapping(0, 300).unwrap().is_empty());
        let history = repository.timeline_undo_history().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].operation_label, "Deleted sessions");
        assert_eq!(history[0].session_count, 1);

        assert_eq!(
            repository
                .undo_timeline_edit(result.undo_token.as_deref().unwrap())
                .unwrap(),
            1
        );
        assert_eq!(repository.records_overlapping(0, 300).unwrap().len(), 1);
    }

    #[test]
    fn split_session_can_be_undone_without_leaving_the_new_half() {
        let repository = repository();
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                started_at_ms: 100,
            })
            .unwrap();
        repository
            .close_session(session.id, 300, ClosedReason::BecameActive)
            .unwrap();

        let result = repository.split_closed_session(session.id, 200).unwrap();
        assert_eq!(
            repository.timeline_undo_history().unwrap()[0].operation_label,
            "Split session"
        );
        let split = repository.records_overlapping(0, 400).unwrap();
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].session.ended_at_ms, 200);
        assert_eq!(split[1].session.started_at_ms, 200);

        repository
            .undo_timeline_edit(result.undo_token.as_deref().unwrap())
            .unwrap();
        let restored = repository.records_overlapping(0, 400).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].session.started_at_ms, 100);
        assert_eq!(restored[0].session.ended_at_ms, 300);
    }

    #[test]
    fn legacy_undo_snapshots_without_a_label_remain_readable() {
        let snapshot: TimelineUndoSnapshot =
            serde_json::from_str(r#"{"sessions":[],"delete_session_ids":[]}"#).unwrap();
        assert_eq!(snapshot.operation_label, None);
    }

    #[test]
    fn import_skip_and_merge_policies_are_transactional() {
        let repository = repository();
        let records = vec![ImportRecord {
            state: ActivityState::Active,
            application_name: Some("Editor".to_owned()),
            bundle_identifier: Some("example.editor".to_owned()),
            started_at_ms: 100,
            ended_at_ms: 200,
            window_title: None,
            note: Some("Imported".to_owned()),
        }];
        assert_eq!(
            repository.import_records(&records, false).unwrap(),
            (1, 0, 0)
        );
        assert_eq!(
            repository.import_records(&records, false).unwrap(),
            (0, 0, 1)
        );
        assert_eq!(
            repository.import_records(&records, true).unwrap(),
            (0, 1, 0)
        );
    }

    #[test]
    #[ignore = "deterministic one-year performance baseline"]
    fn benchmark_one_year_timeline_queries() {
        let repository = repository();
        let application = application(&repository, 0);
        let session_count = 365 * 96;
        {
            let mut connection = repository.database.lock().unwrap();
            let transaction = connection.transaction().unwrap();
            {
                let mut insert = transaction
                    .prepare(
                        "INSERT INTO activity_sessions(
                            state, application_id, started_at_ms, ended_at_ms, duration_ms,
                            is_open, closed_reason, created_at_ms, updated_at_ms
                         ) VALUES ('ACTIVE', ?1, ?2, ?3, ?4, 0, 'APP_CHANGED', ?2, ?3)",
                    )
                    .unwrap();
                for index in 0..session_count {
                    let started_at_ms = index as i64 * 15 * 60_000;
                    let ended_at_ms = started_at_ms + 10 * 60_000;
                    insert
                        .execute(params![
                            application.id,
                            started_at_ms,
                            ended_at_ms,
                            ended_at_ms - started_at_ms
                        ])
                        .unwrap();
                }
            }
            transaction.commit().unwrap();
        }

        let day_start_ms = 180 * 24 * 60 * 60_000_i64;
        let day_end_ms = day_start_ms + 24 * 60 * 60_000_i64;
        let page_started = std::time::Instant::now();
        let page = repository
            .records_overlapping_page(day_start_ms, day_end_ms, 0, 200)
            .unwrap();
        let page_elapsed = page_started.elapsed();

        let year_started = std::time::Instant::now();
        let totals = repository
            .timeline_page_totals(0, 365 * 24 * 60 * 60_000_i64)
            .unwrap();
        let year_elapsed = year_started.elapsed();

        eprintln!(
            "one-year baseline: {session_count} sessions, day page {page_elapsed:?}, year totals {year_elapsed:?}"
        );
        assert_eq!(page.len(), 96);
        assert_eq!(totals.0, session_count);
        assert!(page_elapsed < std::time::Duration::from_secs(2));
        assert!(year_elapsed < std::time::Duration::from_secs(2));
    }
}
