use rusqlite::params;

use crate::{
    database::repository::{ActivityRepository, now_millis},
    error::{AppError, AppResult},
};

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

impl ActivityRepository {
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
}
