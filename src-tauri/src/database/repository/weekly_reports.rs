use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::error::{AppError, AppResult};

use super::ActivityRepository;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyReportArchiveInput {
    pub week_start_date: String,
    pub week_end_date: String,
    pub generated_at_ms: i64,
    pub active_duration_ms: i64,
    pub idle_duration_ms: i64,
    pub previous_week_active_duration_ms: i64,
    pub strongest_day_date: Option<String>,
    pub peak_hour: Option<i64>,
    pub leading_category: Option<String>,
    pub focus_completion_rate: Option<i64>,
    pub payload_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyReportArchive {
    pub week_start_date: String,
    pub week_end_date: String,
    pub generated_at_ms: i64,
    pub active_duration_ms: i64,
    pub idle_duration_ms: i64,
    pub previous_week_active_duration_ms: i64,
    pub strongest_day_date: Option<String>,
    pub peak_hour: Option<i64>,
    pub leading_category: Option<String>,
    pub focus_completion_rate: Option<i64>,
    pub payload_json: String,
    pub notified_at_ms: Option<i64>,
}

impl ActivityRepository {
    pub fn archive_weekly_report(
        &self,
        input: &WeeklyReportArchiveInput,
    ) -> AppResult<WeeklyReportArchive> {
        validate_weekly_report_archive(input)?;
        let connection = self.database.lock()?;
        connection.execute(
            "INSERT INTO weekly_report_archives (
                week_start_date, week_end_date, generated_at_ms, active_duration_ms,
                idle_duration_ms, previous_week_active_duration_ms, strongest_day_date,
                peak_hour, leading_category, focus_completion_rate, payload_json, notified_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)
             ON CONFLICT(week_start_date) DO UPDATE SET
                week_end_date = excluded.week_end_date,
                generated_at_ms = excluded.generated_at_ms,
                active_duration_ms = excluded.active_duration_ms,
                idle_duration_ms = excluded.idle_duration_ms,
                previous_week_active_duration_ms = excluded.previous_week_active_duration_ms,
                strongest_day_date = excluded.strongest_day_date,
                peak_hour = excluded.peak_hour,
                leading_category = excluded.leading_category,
                focus_completion_rate = excluded.focus_completion_rate,
                payload_json = excluded.payload_json",
            params![
                input.week_start_date,
                input.week_end_date,
                input.generated_at_ms,
                input.active_duration_ms,
                input.idle_duration_ms,
                input.previous_week_active_duration_ms,
                input.strongest_day_date,
                input.peak_hour,
                input.leading_category,
                input.focus_completion_rate,
                input.payload_json,
            ],
        )?;
        find_weekly_report_archive(&connection, &input.week_start_date)?.ok_or_else(|| {
            AppError::InvalidSession(
                "weekly report archive could not be read after saving".to_owned(),
            )
        })
    }

    pub fn weekly_report_archives(&self, limit: usize) -> AppResult<Vec<WeeklyReportArchive>> {
        if !(1..=104).contains(&limit) {
            return Err(AppError::InvalidSession(
                "weekly report archive limit must be between 1 and 104".to_owned(),
            ));
        }
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT week_start_date, week_end_date, generated_at_ms, active_duration_ms,
                    idle_duration_ms, previous_week_active_duration_ms, strongest_day_date,
                    peak_hour, leading_category, focus_completion_rate, payload_json, notified_at_ms
             FROM weekly_report_archives
             ORDER BY week_start_date DESC
             LIMIT ?1",
        )?;
        statement
            .query_map([limit as i64], map_weekly_report_archive)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn weekly_report_archive(
        &self,
        week_start_date: &str,
    ) -> AppResult<Option<WeeklyReportArchive>> {
        let connection = self.database.lock()?;
        find_weekly_report_archive(&connection, week_start_date)
    }

    pub fn oldest_unnotified_weekly_report_archive(
        &self,
    ) -> AppResult<Option<WeeklyReportArchive>> {
        let connection = self.database.lock()?;
        connection
            .query_row(
                "SELECT week_start_date, week_end_date, generated_at_ms, active_duration_ms,
                        idle_duration_ms, previous_week_active_duration_ms, strongest_day_date,
                        peak_hour, leading_category, focus_completion_rate, payload_json,
                        notified_at_ms
                 FROM weekly_report_archives
                 WHERE notified_at_ms IS NULL
                 ORDER BY week_start_date ASC
                 LIMIT 1",
                [],
                map_weekly_report_archive,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn mark_weekly_report_notified(
        &self,
        week_start_date: &str,
        notified_at_ms: i64,
    ) -> AppResult<()> {
        let connection = self.database.lock()?;
        let updated = connection.execute(
            "UPDATE weekly_report_archives SET notified_at_ms = ?2 WHERE week_start_date = ?1",
            params![week_start_date, notified_at_ms],
        )?;
        if updated == 0 {
            return Err(AppError::InvalidSession(
                "weekly report archive was not found".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn delete_weekly_report_archive(&self, week_start_date: &str) -> AppResult<()> {
        let connection = self.database.lock()?;
        let deleted = connection.execute(
            "DELETE FROM weekly_report_archives WHERE week_start_date = ?1",
            [week_start_date],
        )?;
        if deleted == 0 {
            return Err(AppError::InvalidSession(
                "weekly report archive was not found".to_owned(),
            ));
        }
        Ok(())
    }
}

fn validate_weekly_report_archive(input: &WeeklyReportArchiveInput) -> AppResult<()> {
    let start = NaiveDate::parse_from_str(&input.week_start_date, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidSession("week start must use YYYY-MM-DD".to_owned()))?;
    let end = NaiveDate::parse_from_str(&input.week_end_date, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidSession("week end must use YYYY-MM-DD".to_owned()))?;
    if end.signed_duration_since(start).num_days() != 6 {
        return Err(AppError::InvalidSession(
            "weekly report archive must cover exactly seven dates".to_owned(),
        ));
    }
    if input.generated_at_ms < 0
        || input.active_duration_ms < 0
        || input.idle_duration_ms < 0
        || input.previous_week_active_duration_ms < 0
        || input
            .peak_hour
            .is_some_and(|hour| !(0..=23).contains(&hour))
        || input
            .focus_completion_rate
            .is_some_and(|rate| !(0..=100).contains(&rate))
        || input.payload_json.len() > 1_000_000
        || serde_json::from_str::<serde_json::Value>(&input.payload_json).is_err()
    {
        return Err(AppError::InvalidSession(
            "weekly report archive contains invalid data".to_owned(),
        ));
    }
    Ok(())
}

fn find_weekly_report_archive(
    connection: &Connection,
    week_start_date: &str,
) -> AppResult<Option<WeeklyReportArchive>> {
    connection
        .query_row(
            "SELECT week_start_date, week_end_date, generated_at_ms, active_duration_ms,
                    idle_duration_ms, previous_week_active_duration_ms, strongest_day_date,
                    peak_hour, leading_category, focus_completion_rate, payload_json, notified_at_ms
             FROM weekly_report_archives WHERE week_start_date = ?1",
            [week_start_date],
            map_weekly_report_archive,
        )
        .optional()
        .map_err(Into::into)
}

fn map_weekly_report_archive(row: &Row<'_>) -> rusqlite::Result<WeeklyReportArchive> {
    Ok(WeeklyReportArchive {
        week_start_date: row.get(0)?,
        week_end_date: row.get(1)?,
        generated_at_ms: row.get(2)?,
        active_duration_ms: row.get(3)?,
        idle_duration_ms: row.get(4)?,
        previous_week_active_duration_ms: row.get(5)?,
        strongest_day_date: row.get(6)?,
        peak_hour: row.get(7)?,
        leading_category: row.get(8)?,
        focus_completion_rate: row.get(9)?,
        payload_json: row.get(10)?,
        notified_at_ms: row.get(11)?,
    })
}
