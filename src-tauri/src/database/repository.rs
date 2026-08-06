use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::Path,
    time::Duration,
};

use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, Row, backup::Backup, params, params_from_iter};

use crate::{
    activity::{
        ActivitySession, ActivityState, Application, ClosedReason, NewApplication, NewSession,
    },
    error::{AppError, AppResult},
};

use super::Database;

mod focus_plans;
mod projects;
mod weekly_reports;

pub use focus_plans::{FocusPlanHistoryEntry, FocusPlanTemplate, PersistedFocusMode};
pub use projects::{
    ActivityTag, ActivityTagInput, Project, ProjectInput, SessionOrganization, SessionTagUpdateMode,
};
pub use weekly_reports::{WeeklyReportArchive, WeeklyReportArchiveInput};

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
    pub automatic_encrypted_backup_enabled: bool,
    pub last_encrypted_backup_at_ms: i64,
    pub weekly_report_auto_archive_enabled: bool,
    pub weekly_report_notification_enabled: bool,
    pub weekly_report_notification_weekday: i64,
    pub weekly_report_notification_time: String,
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
#[serde(rename_all = "camelCase")]
pub struct ActivityRecord {
    pub session: ActivitySession,
    pub application: Option<Application>,
    pub effective_category: Option<String>,
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
    pub project_id: Option<i64>,
    pub tag_id: Option<i64>,
    #[serde(default)]
    pub unassigned_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CategoryRuleMatchField {
    ApplicationName,
    BundleId,
    WindowTitle,
}

impl CategoryRuleMatchField {
    fn as_database_value(self) -> &'static str {
        match self {
            Self::ApplicationName => "APPLICATION_NAME",
            Self::BundleId => "BUNDLE_ID",
            Self::WindowTitle => "WINDOW_TITLE",
        }
    }

    fn from_database_value(value: &str) -> rusqlite::Result<Self> {
        match value {
            "APPLICATION_NAME" => Ok(Self::ApplicationName),
            "BUNDLE_ID" => Ok(Self::BundleId),
            "WINDOW_TITLE" => Ok(Self::WindowTitle),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRuleInput {
    pub match_field: CategoryRuleMatchField,
    pub pattern: String,
    pub category: String,
    pub priority: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRule {
    pub id: i64,
    pub match_field: CategoryRuleMatchField,
    pub pattern: String,
    pub category: String,
    pub priority: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRulePreviewSample {
    pub application_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub would_apply: bool,
    pub shadowed_by_rule_id: Option<i64>,
    pub shadowed_by_category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRuleConflict {
    pub rule_id: i64,
    pub match_field: CategoryRuleMatchField,
    pub pattern: String,
    pub category: String,
    pub priority: i64,
    pub session_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRulePreview {
    pub matched_session_count: usize,
    pub matched_application_count: usize,
    pub effective_session_count: usize,
    pub shadowed_session_count: usize,
    pub conflicts: Vec<CategoryRuleConflict>,
    pub samples: Vec<CategoryRulePreviewSample>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRulesReapplyPreviewSample {
    pub application_name: String,
    pub window_title: Option<String>,
    pub previous_category: String,
    pub next_category: String,
    pub previous_is_override: bool,
    pub next_is_override: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRulesReapplyPreview {
    pub scanned_session_count: usize,
    pub affected_session_count: usize,
    pub category_change_count: usize,
    pub assigned_session_count: usize,
    pub cleared_session_count: usize,
    pub samples: Vec<CategoryRulesReapplyPreviewSample>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRulesReapplyResult {
    pub affected_count: usize,
    pub undo_token: Option<String>,
    pub undo_created_at_ms: Option<i64>,
    pub undo_expires_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRulesReapplyUndoStatus {
    pub token: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub affected_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UsageLimitScopeType {
    Application,
    Category,
}

impl UsageLimitScopeType {
    fn as_database_value(self) -> &'static str {
        match self {
            Self::Application => "APPLICATION",
            Self::Category => "CATEGORY",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitRule {
    pub id: i64,
    pub scope_type: UsageLimitScopeType,
    pub application_id: Option<i64>,
    pub application_name: Option<String>,
    pub category: Option<String>,
    pub weekday_limit_minutes: i64,
    pub weekend_limit_minutes: i64,
    pub notifications_enabled: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitRuleInput {
    pub scope_type: UsageLimitScopeType,
    pub application_id: Option<i64>,
    pub category: Option<String>,
    pub weekday_limit_minutes: i64,
    pub weekend_limit_minutes: i64,
    pub notifications_enabled: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitApplicationTarget {
    pub application_id: i64,
    pub application_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitTargets {
    pub applications: Vec<UsageLimitApplicationTarget>,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitDailyException {
    pub rule_id: i64,
    pub local_date: String,
    pub temporary_added_minutes: i64,
    pub notifications_snoozed_until_ms: Option<i64>,
    pub notifications_silenced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitReminderHistoryEntry {
    pub rule_id: i64,
    pub scope_type: UsageLimitScopeType,
    pub application_id: Option<i64>,
    pub application_name: Option<String>,
    pub category: Option<String>,
    pub target_name: String,
    pub local_date: String,
    pub threshold: i64,
    pub delivered_at_ms: i64,
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
    sessions: Vec<DataHealthUndoSession>,
    #[serde(default)]
    organizations: Vec<SessionOrganizationSnapshot>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DataHealthUndoSession {
    #[serde(flatten)]
    session: ActivitySession,
    #[serde(default)]
    category_override: Option<String>,
}

const SESSION_QUERY_BATCH_SIZE: usize = 500;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct SessionOrganizationSnapshot {
    session_id: i64,
    project_id: Option<i64>,
    tag_ids: Vec<i64>,
}

fn session_organization_snapshots(
    connection: &Connection,
    session_ids: &[i64],
) -> AppResult<Vec<SessionOrganizationSnapshot>> {
    let session_ids = session_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut snapshots = session_ids
        .iter()
        .map(|session_id| {
            (
                *session_id,
                SessionOrganizationSnapshot {
                    session_id: *session_id,
                    project_id: None,
                    tag_ids: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let session_ids = session_ids.into_iter().collect::<Vec<_>>();
    for batch in session_ids.chunks(SESSION_QUERY_BATCH_SIZE) {
        let placeholders = std::iter::repeat_n("?", batch.len())
            .collect::<Vec<_>>()
            .join(",");
        let mut project_statement = connection.prepare(&format!(
            "SELECT session_id, project_id FROM session_projects
             WHERE session_id IN ({placeholders})"
        ))?;
        let projects = project_statement
            .query_map(params_from_iter(batch.iter()), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (session_id, project_id) in projects {
            snapshots
                .get_mut(&session_id)
                .expect("queried project must belong to the requested batch")
                .project_id = Some(project_id);
        }

        let mut tag_statement = connection.prepare(&format!(
            "SELECT session_id, tag_id FROM session_tags
             WHERE session_id IN ({placeholders})
             ORDER BY session_id, tag_id"
        ))?;
        let tags = tag_statement
            .query_map(params_from_iter(batch.iter()), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (session_id, tag_id) in tags {
            snapshots
                .get_mut(&session_id)
                .expect("queried tag must belong to the requested batch")
                .tag_ids
                .push(tag_id);
        }
    }
    Ok(snapshots.into_values().collect())
}

fn closed_session_snapshots(
    connection: &Connection,
    session_ids: &[i64],
) -> AppResult<Vec<ActivitySession>> {
    let session_ids = session_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut sessions = BTreeMap::new();
    let batched_ids = session_ids.iter().copied().collect::<Vec<_>>();
    for batch in batched_ids.chunks(SESSION_QUERY_BATCH_SIZE) {
        let placeholders = std::iter::repeat_n("?", batch.len())
            .collect::<Vec<_>>()
            .join(",");
        let mut statement =
            connection.prepare(&format!("{SESSION_SELECT} WHERE id IN ({placeholders})"))?;
        let batch_sessions = statement
            .query_map(params_from_iter(batch.iter()), map_session)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        sessions.extend(
            batch_sessions
                .into_iter()
                .map(|session| (session.id, session)),
        );
    }

    session_ids
        .into_iter()
        .map(|session_id| {
            let session = sessions
                .remove(&session_id)
                .ok_or(AppError::SessionNotFound(session_id))?;
            if session.is_open {
                return Err(AppError::InvalidSession(
                    "open sessions cannot be changed".to_owned(),
                ));
            }
            Ok(session)
        })
        .collect()
}

fn restore_session_organization_snapshots(
    connection: &Connection,
    snapshots: &[SessionOrganizationSnapshot],
) -> AppResult<()> {
    for snapshot in snapshots {
        if let Some(project_id) = snapshot.project_id {
            connection.execute(
                "INSERT INTO session_projects (session_id, project_id) VALUES (?1, ?2)",
                params![snapshot.session_id, project_id],
            )?;
        }
        for tag_id in &snapshot.tag_ids {
            connection.execute(
                "INSERT INTO session_tags (session_id, tag_id) VALUES (?1, ?2)",
                params![snapshot.session_id, tag_id],
            )?;
        }
    }
    Ok(())
}

fn session_ids_matching_snapshots(
    connection: &Connection,
    snapshots: &[ActivitySession],
) -> AppResult<BTreeSet<i64>> {
    let expected_created_at_ms = snapshots
        .iter()
        .map(|session| (session.id, session.created_at_ms))
        .collect::<BTreeMap<_, _>>();
    let mut matching_ids = BTreeSet::new();
    let session_ids = expected_created_at_ms.keys().copied().collect::<Vec<_>>();

    for batch in session_ids.chunks(SESSION_QUERY_BATCH_SIZE) {
        let placeholders = std::iter::repeat_n("?", batch.len())
            .collect::<Vec<_>>()
            .join(",");
        let mut statement = connection.prepare(&format!(
            "SELECT id, created_at_ms FROM activity_sessions WHERE id IN ({placeholders})"
        ))?;
        let sessions = statement
            .query_map(params_from_iter(batch.iter()), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (session_id, created_at_ms) in sessions {
            if expected_created_at_ms.get(&session_id) == Some(&created_at_ms) {
                matching_ids.insert(session_id);
            }
        }
    }

    Ok(matching_ids)
}

#[derive(Debug, Clone, Copy)]
struct HealthSessionRange {
    id: i64,
    started_at_ms: i64,
    ended_at_ms: i64,
}

const CLOSED_HEALTH_RANGES_QUERY: &str =
    "SELECT id, started_at_ms, ended_at_ms FROM activity_sessions WHERE is_open = 0";
const CLOSED_HEALTH_SESSIONS_QUERY: &str = "SELECT
    id, state, application_id, window_title, started_at_ms, ended_at_ms,
    duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms, note,
    category_override
    FROM activity_sessions WHERE is_open = 0";

fn overlapping_session_ids(mut ranges: Vec<HealthSessionRange>) -> HashSet<i64> {
    ranges.sort_unstable_by_key(|range| (range.started_at_ms, range.id));
    let mut overlapping = HashSet::new();
    let mut prefix_max_end: Option<(i64, i64)> = None;
    let mut group_start = 0;

    while group_start < ranges.len() {
        let started_at_ms = ranges[group_start].started_at_ms;
        let mut group_end = group_start + 1;
        while group_end < ranges.len() && ranges[group_end].started_at_ms == started_at_ms {
            group_end += 1;
        }
        let group = &ranges[group_start..group_end];

        if let Some((maximum_end, maximum_id)) = prefix_max_end
            && maximum_end > started_at_ms
        {
            overlapping.insert(maximum_id);
            overlapping.extend(group.iter().map(|range| range.id));
        }

        let positive_count = group
            .iter()
            .filter(|range| range.ended_at_ms > started_at_ms)
            .count();
        if positive_count > 1 {
            overlapping.extend(
                group
                    .iter()
                    .filter(|range| range.ended_at_ms > started_at_ms)
                    .map(|range| range.id),
            );
        }

        if let Some(group_maximum) = group.iter().max_by_key(|range| range.ended_at_ms)
            && prefix_max_end.is_none_or(|(maximum_end, _)| group_maximum.ended_at_ms > maximum_end)
        {
            prefix_max_end = Some((group_maximum.ended_at_ms, group_maximum.id));
        }
        group_start = group_end;
    }

    overlapping
}

fn closed_health_ranges(connection: &Connection) -> AppResult<Vec<HealthSessionRange>> {
    let mut statement = connection.prepare(CLOSED_HEALTH_RANGES_QUERY)?;
    Ok(statement
        .query_map([], |row| {
            Ok(HealthSessionRange {
                id: row.get(0)?,
                started_at_ms: row.get(1)?,
                ended_at_ms: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

type OverlappingSession = (i64, String, Option<i64>, Option<String>, i64, i64);

const CATEGORY_RULES_REAPPLY_UNDO_TTL_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(serde::Serialize, serde::Deserialize)]
struct TimelineUndoSnapshot {
    sessions: Vec<ActivitySession>,
    delete_session_ids: Vec<i64>,
    #[serde(default)]
    organizations: Vec<SessionOrganizationSnapshot>,
    #[serde(default)]
    organization_only: bool,
    #[serde(default)]
    operation_label: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CategoryRulesReapplyUndoSnapshot {
    changes: Vec<CategoryRulesReapplyUndoChange>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CategoryRulesReapplyUndoChange {
    session_id: i64,
    previous_category_override: Option<String>,
    applied_category_override: Option<String>,
}

struct CategoryRulesReapplyChange {
    session_id: i64,
    application_name: String,
    window_title: Option<String>,
    application_category: String,
    previous_category_override: Option<String>,
    next_category_override: Option<String>,
}

struct CategoryRuleSession {
    session_id: i64,
    application_id: i64,
    application_name: String,
    bundle_id: Option<String>,
    window_title: Option<String>,
    application_category: String,
    category_override: Option<String>,
}

struct PreparedCategoryRule<'a> {
    rule: &'a CategoryRule,
    normalized_pattern: String,
}

struct NormalizedCategoryCandidates {
    application_name: String,
    bundle_id: Option<String>,
    window_title: Option<String>,
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

    pub fn category_rules(&self) -> AppResult<Vec<CategoryRule>> {
        let connection = self.database.lock()?;
        category_rules_from_connection(&connection)
    }

    pub fn reorder_category_rules(&self, rule_ids: &[i64]) -> AppResult<Vec<CategoryRule>> {
        if rule_ids.len() > 10_000 {
            return Err(AppError::InvalidSession(
                "no more than 10000 category rules can be ordered".to_owned(),
            ));
        }
        let requested_ids = rule_ids.iter().copied().collect::<HashSet<_>>();
        if requested_ids.len() != rule_ids.len() {
            return Err(AppError::InvalidSession(
                "category rule order contains duplicate rules".to_owned(),
            ));
        }

        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let existing_rules = category_rules_from_connection(&transaction)?;
        let existing_ids = existing_rules
            .iter()
            .map(|rule| rule.id)
            .collect::<HashSet<_>>();
        if requested_ids != existing_ids {
            return Err(AppError::InvalidSession(
                "category rule order must contain every rule exactly once".to_owned(),
            ));
        }

        let updated_at_ms = now_millis();
        for (priority, rule_id) in rule_ids.iter().enumerate() {
            transaction.execute(
                "UPDATE category_rules SET priority = ?2, updated_at_ms = ?3 WHERE id = ?1",
                params![rule_id, priority as i64, updated_at_ms],
            )?;
        }
        let reordered = category_rules_from_connection(&transaction)?;
        transaction.commit()?;
        Ok(reordered)
    }

    pub fn create_category_rule(&self, input: &CategoryRuleInput) -> AppResult<CategoryRule> {
        let normalized = normalize_category_rule(input)?;
        let now_ms = now_millis();
        let connection = self.database.lock()?;
        connection.execute(
            "INSERT INTO category_rules (
                match_field, pattern, category, priority, enabled, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![
                normalized.match_field.as_database_value(),
                normalized.pattern,
                normalized.category,
                normalized.priority,
                normalized.enabled,
                now_ms,
            ],
        )?;
        find_category_rule(&connection, connection.last_insert_rowid())?.ok_or_else(|| {
            AppError::InvalidSession("created category rule was not found".to_owned())
        })
    }

    pub fn update_category_rule(
        &self,
        rule_id: i64,
        input: &CategoryRuleInput,
    ) -> AppResult<CategoryRule> {
        let normalized = normalize_category_rule(input)?;
        let connection = self.database.lock()?;
        let changed = connection.execute(
            "UPDATE category_rules
             SET match_field = ?2, pattern = ?3, category = ?4,
                 priority = ?5, enabled = ?6, updated_at_ms = ?7
             WHERE id = ?1",
            params![
                rule_id,
                normalized.match_field.as_database_value(),
                normalized.pattern,
                normalized.category,
                normalized.priority,
                normalized.enabled,
                now_millis(),
            ],
        )?;
        if changed == 0 {
            return Err(AppError::InvalidSession(
                "category rule was not found".to_owned(),
            ));
        }
        find_category_rule(&connection, rule_id)?.ok_or_else(|| {
            AppError::InvalidSession("updated category rule was not found".to_owned())
        })
    }

    pub fn delete_category_rule(&self, rule_id: i64) -> AppResult<()> {
        let connection = self.database.lock()?;
        if connection.execute("DELETE FROM category_rules WHERE id = ?1", [rule_id])? == 0 {
            return Err(AppError::InvalidSession(
                "category rule was not found".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn resolve_category_rule(
        &self,
        application_name: &str,
        bundle_id: Option<&str>,
        window_title: Option<&str>,
    ) -> AppResult<Option<String>> {
        let connection = self.database.lock()?;
        let rules = enabled_category_rules(&connection)?;
        Ok(resolve_category_from_rules(
            &rules,
            application_name,
            bundle_id,
            window_title,
        ))
    }

    pub fn preview_category_rule(
        &self,
        input: &CategoryRuleInput,
        editing_rule_id: Option<i64>,
    ) -> AppResult<CategoryRulePreview> {
        const SAMPLE_LIMIT: usize = 5;

        let normalized = normalize_category_rule(input)?;
        let connection = self.database.lock()?;
        if let Some(rule_id) = editing_rule_id
            && find_category_rule(&connection, rule_id)?.is_none()
        {
            return Err(AppError::InvalidSession(
                "category rule was not found".to_owned(),
            ));
        }

        let preceding_rules = enabled_category_rules(&connection)?
            .into_iter()
            .filter(|rule| {
                Some(rule.id) != editing_rule_id
                    && (rule.priority < normalized.priority
                        || (rule.priority == normalized.priority
                            && editing_rule_id.is_none_or(|rule_id| rule.id < rule_id)))
            })
            .collect::<Vec<_>>();

        let sessions = category_rule_sessions(&connection)?;
        drop(connection);
        let normalized_pattern = normalized.pattern.to_lowercase();
        let prepared_preceding_rules = prepare_category_rules(&preceding_rules);
        let mut matched_application_ids = HashSet::new();
        let mut matched_session_count = 0;
        let mut effective_session_count = 0;
        let mut shadowed_session_count = 0;
        let mut conflicts = Vec::<CategoryRuleConflict>::new();
        let mut samples = Vec::with_capacity(SAMPLE_LIMIT);

        for session in sessions {
            let candidates = NormalizedCategoryCandidates::new(
                &session.application_name,
                session.bundle_id.as_deref(),
                session.window_title.as_deref(),
            );
            if !category_rule_matches_normalized(
                normalized.match_field,
                &normalized_pattern,
                &candidates,
            ) {
                continue;
            }

            matched_session_count += 1;
            matched_application_ids.insert(session.application_id);
            let shadowing_rule = prepared_preceding_rules
                .iter()
                .find(|prepared| prepared.matches(&candidates))
                .map(|prepared| prepared.rule);

            if let Some(rule) = shadowing_rule {
                shadowed_session_count += 1;
                if let Some(conflict) = conflicts
                    .iter_mut()
                    .find(|conflict| conflict.rule_id == rule.id)
                {
                    conflict.session_count += 1;
                } else {
                    conflicts.push(CategoryRuleConflict {
                        rule_id: rule.id,
                        match_field: rule.match_field,
                        pattern: rule.pattern.clone(),
                        category: rule.category.clone(),
                        priority: rule.priority,
                        session_count: 1,
                    });
                }
            } else if normalized.enabled {
                effective_session_count += 1;
            }

            if samples.len() < SAMPLE_LIMIT {
                samples.push(CategoryRulePreviewSample {
                    application_name: truncate_preview_text(&session.application_name),
                    bundle_id: session.bundle_id.as_deref().map(truncate_preview_text),
                    window_title: session.window_title.as_deref().map(truncate_preview_text),
                    would_apply: normalized.enabled && shadowing_rule.is_none(),
                    shadowed_by_rule_id: shadowing_rule.map(|rule| rule.id),
                    shadowed_by_category: shadowing_rule
                        .map(|rule| truncate_preview_text(&rule.category)),
                });
            }
        }

        conflicts.sort_by_key(|conflict| (conflict.priority, conflict.rule_id));
        Ok(CategoryRulePreview {
            matched_session_count,
            matched_application_count: matched_application_ids.len(),
            effective_session_count,
            shadowed_session_count,
            conflicts,
            samples,
        })
    }

    pub fn preview_category_rules_reapply(&self) -> AppResult<CategoryRulesReapplyPreview> {
        const SAMPLE_LIMIT: usize = 5;

        let (rules, sessions) = {
            let connection = self.database.lock()?;
            (
                enabled_category_rules(&connection)?,
                category_rule_sessions(&connection)?,
            )
        };
        let (scanned_session_count, changes) = category_rules_reapply_changes(sessions, &rules);
        let category_change_count = changes
            .iter()
            .filter(|change| change.previous_category() != change.next_category())
            .count();
        let assigned_session_count = changes
            .iter()
            .filter(|change| change.next_category_override.is_some())
            .count();
        let cleared_session_count = changes
            .iter()
            .filter(|change| change.next_category_override.is_none())
            .count();
        let samples = changes
            .iter()
            .take(SAMPLE_LIMIT)
            .map(|change| CategoryRulesReapplyPreviewSample {
                application_name: truncate_preview_text(&change.application_name),
                window_title: change.window_title.as_deref().map(truncate_preview_text),
                previous_category: truncate_preview_text(change.previous_category()),
                next_category: truncate_preview_text(change.next_category()),
                previous_is_override: change.previous_category_override.is_some(),
                next_is_override: change.next_category_override.is_some(),
            })
            .collect();
        Ok(CategoryRulesReapplyPreview {
            scanned_session_count,
            affected_session_count: changes.len(),
            category_change_count,
            assigned_session_count,
            cleared_session_count,
            samples,
        })
    }

    pub fn reapply_category_rules(&self) -> AppResult<CategoryRulesReapplyResult> {
        let (rules, sessions) = {
            let connection = self.database.lock()?;
            (
                enabled_category_rules(&connection)?,
                category_rule_sessions(&connection)?,
            )
        };
        let (_, changes) = category_rules_reapply_changes(sessions, &rules);
        if changes.is_empty() {
            return Ok(CategoryRulesReapplyResult {
                affected_count: 0,
                undo_token: None,
                undo_created_at_ms: None,
                undo_expires_at_ms: None,
            });
        }

        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        if enabled_category_rules(&transaction)? != rules {
            return Err(AppError::InvalidSession(
                "category rules changed while reclassification was being prepared; preview again"
                    .to_owned(),
            ));
        }

        let mut applied_changes = Vec::with_capacity(changes.len());
        {
            let mut update = transaction.prepare_cached(
                "UPDATE activity_sessions SET category_override = ?2
                 WHERE id = ?1 AND category_override IS ?3",
            )?;
            for change in changes {
                let updated = update.execute(params![
                    change.session_id,
                    change.next_category_override,
                    change.previous_category_override,
                ])?;
                if updated == 1 {
                    applied_changes.push(change);
                }
            }
        }
        if applied_changes.is_empty() {
            transaction.commit()?;
            return Ok(CategoryRulesReapplyResult {
                affected_count: 0,
                undo_token: None,
                undo_created_at_ms: None,
                undo_expires_at_ms: None,
            });
        }

        let now_ms = now_millis();
        let token = transaction.query_row("SELECT lower(hex(randomblob(16)))", [], |row| {
            row.get::<_, String>(0)
        })?;
        let snapshot = CategoryRulesReapplyUndoSnapshot {
            changes: applied_changes
                .iter()
                .map(|change| CategoryRulesReapplyUndoChange {
                    session_id: change.session_id,
                    previous_category_override: change.previous_category_override.clone(),
                    applied_category_override: change.next_category_override.clone(),
                })
                .collect(),
        };
        transaction.execute("DELETE FROM category_rule_reapply_undo", [])?;
        transaction.execute(
            "INSERT INTO category_rule_reapply_undo(token, snapshot_json, created_at_ms)
             VALUES (?1, ?2, ?3)",
            params![
                token,
                serde_json::to_string(&snapshot).map_err(|error| {
                    AppError::InvalidSession(format!(
                        "could not create category reclassification undo snapshot: {error}"
                    ))
                })?,
                now_ms,
            ],
        )?;
        transaction.commit()?;
        Ok(CategoryRulesReapplyResult {
            affected_count: applied_changes.len(),
            undo_token: Some(token),
            undo_created_at_ms: Some(now_ms),
            undo_expires_at_ms: Some(now_ms.saturating_add(CATEGORY_RULES_REAPPLY_UNDO_TTL_MS)),
        })
    }

    pub fn category_rules_reapply_undo_status(
        &self,
    ) -> AppResult<Option<CategoryRulesReapplyUndoStatus>> {
        let connection = self.database.lock()?;
        let now_ms = now_millis();
        connection.execute(
            "DELETE FROM category_rule_reapply_undo WHERE created_at_ms < ?1",
            [now_ms.saturating_sub(CATEGORY_RULES_REAPPLY_UNDO_TTL_MS)],
        )?;
        let row = connection
            .query_row(
                "SELECT token, snapshot_json, created_at_ms
                 FROM category_rule_reapply_undo ORDER BY created_at_ms DESC LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(token, snapshot_json, created_at_ms)| {
            let snapshot: CategoryRulesReapplyUndoSnapshot = serde_json::from_str(&snapshot_json)
                .map_err(|error| {
                AppError::InvalidSession(format!(
                    "invalid category reclassification undo snapshot: {error}"
                ))
            })?;
            Ok(CategoryRulesReapplyUndoStatus {
                token,
                created_at_ms,
                expires_at_ms: created_at_ms.saturating_add(CATEGORY_RULES_REAPPLY_UNDO_TTL_MS),
                affected_count: snapshot.changes.len(),
            })
        })
        .transpose()
    }

    pub fn undo_category_rules_reapply(&self, token: &str) -> AppResult<usize> {
        if token.is_empty() || token.len() > 128 {
            return Err(AppError::InvalidSession(
                "invalid category reclassification undo token".to_owned(),
            ));
        }
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let now_ms = now_millis();
        transaction.execute(
            "DELETE FROM category_rule_reapply_undo WHERE created_at_ms < ?1",
            [now_ms.saturating_sub(CATEGORY_RULES_REAPPLY_UNDO_TTL_MS)],
        )?;
        let snapshot_json = transaction
            .query_row(
                "SELECT snapshot_json FROM category_rule_reapply_undo WHERE token = ?1",
                [token],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::InvalidSession(
                    "category reclassification undo is no longer available".to_owned(),
                )
            })?;
        let snapshot: CategoryRulesReapplyUndoSnapshot = serde_json::from_str(&snapshot_json)
            .map_err(|error| {
                AppError::InvalidSession(format!(
                    "invalid category reclassification undo snapshot: {error}"
                ))
            })?;
        let mut restored_count = 0;
        for change in &snapshot.changes {
            restored_count += transaction.execute(
                "UPDATE activity_sessions SET category_override = ?2
                 WHERE id = ?1 AND category_override IS ?3",
                params![
                    change.session_id,
                    change.previous_category_override,
                    change.applied_category_override,
                ],
            )?;
        }
        transaction.execute(
            "DELETE FROM category_rule_reapply_undo WHERE token = ?1",
            [token],
        )?;
        transaction.commit()?;
        Ok(restored_count)
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
                        break_reminder_minutes, quiet_hours_start, quiet_hours_end,
                        automatic_encrypted_backup_enabled,
                        last_encrypted_backup_at_ms,
                        weekly_report_auto_archive_enabled,
                        weekly_report_notification_enabled,
                        weekly_report_notification_weekday,
                        weekly_report_notification_time
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
                        automatic_encrypted_backup_enabled: row.get(20)?,
                        last_encrypted_backup_at_ms: row.get(21)?,
                        weekly_report_auto_archive_enabled: row.get(22)?,
                        weekly_report_notification_enabled: row.get(23)?,
                        weekly_report_notification_weekday: row.get(24)?,
                        weekly_report_notification_time: row.get(25)?,
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

    pub fn usage_limit_rules(&self) -> AppResult<Vec<UsageLimitRule>> {
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT r.id, r.scope_type, r.application_id, a.name, r.category,
                    r.weekday_limit_minutes, r.weekend_limit_minutes,
                    r.notifications_enabled, r.enabled
             FROM usage_limit_rules r
             LEFT JOIN applications a ON a.id = r.application_id
             ORDER BY r.scope_type, COALESCE(a.name, r.category) COLLATE NOCASE, r.id",
        )?;
        statement
            .query_map([], map_usage_limit_rule)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn usage_limit_targets(&self) -> AppResult<UsageLimitTargets> {
        let connection = self.database.lock()?;
        let applications = {
            let mut statement = connection.prepare(
                "SELECT id, name FROM applications
                 ORDER BY name COLLATE NOCASE, id",
            )?;
            statement
                .query_map([], |row| {
                    Ok(UsageLimitApplicationTarget {
                        application_id: row.get(0)?,
                        application_name: row.get(1)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let categories = {
            let mut statement = connection.prepare(
                "SELECT category FROM (
                    SELECT category FROM applications WHERE length(trim(category)) > 0
                    UNION
                    SELECT category FROM category_rules WHERE length(trim(category)) > 0
                 )
                 ORDER BY category COLLATE NOCASE",
            )?;
            statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(UsageLimitTargets {
            applications,
            categories,
        })
    }

    pub fn create_usage_limit(
        &self,
        input: &UsageLimitRuleInput,
        now_ms: i64,
    ) -> AppResult<UsageLimitRule> {
        let normalized = validate_usage_limit_input(input)?;
        let connection = self.database.lock()?;
        validate_usage_limit_target(&connection, &normalized, None)?;
        connection.execute(
            "INSERT INTO usage_limit_rules (
                scope_type, application_id, category, weekday_limit_minutes,
                weekend_limit_minutes, notifications_enabled, enabled,
                created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                normalized.scope_type.as_database_value(),
                normalized.application_id,
                normalized.category,
                normalized.weekday_limit_minutes,
                normalized.weekend_limit_minutes,
                normalized.notifications_enabled,
                normalized.enabled,
                now_ms,
            ],
        )?;
        usage_limit_rule_by_id(&connection, connection.last_insert_rowid())?
            .ok_or_else(|| AppError::InvalidTimeRange("usage limit was not created".to_owned()))
    }

    pub fn update_usage_limit(
        &self,
        id: i64,
        input: &UsageLimitRuleInput,
        now_ms: i64,
    ) -> AppResult<UsageLimitRule> {
        let normalized = validate_usage_limit_input(input)?;
        let mut connection = self.database.lock()?;
        let previous = usage_limit_rule_by_id(&connection, id)?.ok_or_else(|| {
            AppError::InvalidTimeRange("usage limit rule was not found".to_owned())
        })?;
        validate_usage_limit_target(&connection, &normalized, Some(id))?;
        let alert_basis_changed = previous.scope_type != normalized.scope_type
            || previous.application_id != normalized.application_id
            || !optional_text_eq_ignore_ascii_case(
                previous.category.as_deref(),
                normalized.category.as_deref(),
            )
            || previous.weekday_limit_minutes != normalized.weekday_limit_minutes
            || previous.weekend_limit_minutes != normalized.weekend_limit_minutes;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE usage_limit_rules
             SET scope_type = ?1, application_id = ?2, category = ?3,
                 weekday_limit_minutes = ?4, weekend_limit_minutes = ?5,
                 notifications_enabled = ?6, enabled = ?7, updated_at_ms = ?8
             WHERE id = ?9",
            params![
                normalized.scope_type.as_database_value(),
                normalized.application_id,
                normalized.category,
                normalized.weekday_limit_minutes,
                normalized.weekend_limit_minutes,
                normalized.notifications_enabled,
                normalized.enabled,
                now_ms,
                id,
            ],
        )?;
        if alert_basis_changed {
            transaction.execute("DELETE FROM usage_limit_alerts WHERE rule_id = ?1", [id])?;
        }
        transaction.commit()?;
        usage_limit_rule_by_id(&connection, id)?
            .ok_or_else(|| AppError::InvalidTimeRange("usage limit was not found".to_owned()))
    }

    pub fn delete_usage_limit(&self, id: i64) -> AppResult<()> {
        let connection = self.database.lock()?;
        if connection.execute("DELETE FROM usage_limit_rules WHERE id = ?1", [id])? == 0 {
            return Err(AppError::InvalidTimeRange(
                "usage limit rule was not found".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn active_usage_duration_for_rule(
        &self,
        rule: &UsageLimitRule,
        range_start_ms: i64,
        range_end_ms: i64,
    ) -> AppResult<i64> {
        if range_end_ms <= range_start_ms {
            return Err(AppError::InvalidTimeRange(
                "usage limit range end must be after its start".to_owned(),
            ));
        }
        let connection = self.database.lock()?;
        let (target_column, target): (&str, rusqlite::types::Value) = match rule.scope_type {
            UsageLimitScopeType::Application => (
                "s.application_id",
                rule.application_id
                    .ok_or_else(|| {
                        AppError::InvalidTimeRange(
                            "application usage limit has no application".to_owned(),
                        )
                    })?
                    .into(),
            ),
            UsageLimitScopeType::Category => (
                "COALESCE(s.category_override, a.category) COLLATE NOCASE",
                rule.category
                    .clone()
                    .ok_or_else(|| {
                        AppError::InvalidTimeRange(
                            "category usage limit has no category".to_owned(),
                        )
                    })?
                    .into(),
            ),
        };
        let sql = format!(
            "SELECT COALESCE(SUM(
                MIN(s.ended_at_ms, ?2) - MAX(s.started_at_ms, ?1)
             ), 0)
             FROM activity_sessions s
             JOIN applications a ON a.id = s.application_id
             WHERE s.state = 'ACTIVE'
               AND s.started_at_ms < ?2
               AND s.ended_at_ms > ?1
               AND {target_column} = ?3"
        );
        connection
            .query_row(&sql, params![range_start_ms, range_end_ms, target], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(Into::into)
    }

    pub fn delivered_usage_limit_thresholds(
        &self,
        rule_id: i64,
        local_date: &str,
    ) -> AppResult<Vec<i64>> {
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT threshold FROM usage_limit_alerts
             WHERE rule_id = ?1 AND local_date = ?2
             ORDER BY threshold",
        )?;
        statement
            .query_map(params![rule_id, local_date], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn mark_usage_limit_alerts_delivered(
        &self,
        rule_id: i64,
        local_date: &str,
        thresholds: &[i64],
        delivered_at_ms: i64,
    ) -> AppResult<()> {
        if thresholds
            .iter()
            .any(|threshold| !matches!(threshold, 80 | 100))
        {
            return Err(AppError::InvalidTimeRange(
                "usage limit alert threshold must be 80 or 100".to_owned(),
            ));
        }
        validate_usage_limit_local_date(local_date)?;
        let mut connection = self.database.lock()?;
        let target =
            usage_limit_reminder_target_by_rule_id(&connection, rule_id)?.ok_or_else(|| {
                AppError::InvalidTimeRange("usage limit rule was not found".to_owned())
            })?;
        let transaction = connection.transaction()?;
        for threshold in thresholds {
            let inserted = transaction.execute(
                "INSERT OR IGNORE INTO usage_limit_alerts (
                    rule_id, local_date, threshold, delivered_at_ms
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![rule_id, local_date, threshold, delivered_at_ms],
            )?;
            if inserted == 1 {
                transaction.execute(
                    "INSERT INTO usage_limit_reminder_history (
                        rule_id, scope_type, application_id, application_name, category,
                        target_name, local_date, threshold, delivered_at_ms
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        rule_id,
                        target.scope_type,
                        target.application_id,
                        target.application_name,
                        target.category,
                        target.target_name,
                        local_date,
                        threshold,
                        delivered_at_ms,
                    ],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn usage_limit_daily_exception(
        &self,
        rule_id: i64,
        local_date: &str,
    ) -> AppResult<UsageLimitDailyException> {
        validate_usage_limit_local_date(local_date)?;
        let connection = self.database.lock()?;
        usage_limit_daily_exception_by_rule_and_date(&connection, rule_id, local_date)
    }

    pub fn snooze_usage_limit_notifications(
        &self,
        rule_id: i64,
        local_date: &str,
        minutes: i64,
        now_ms: i64,
        day_start_ms: i64,
        day_end_ms: i64,
    ) -> AppResult<UsageLimitDailyException> {
        if !(5..=1_440).contains(&minutes) {
            return Err(AppError::InvalidTimeRange(
                "usage limit notification snooze must be between 5 and 1440 minutes".to_owned(),
            ));
        }
        validate_usage_limit_local_date(local_date)?;
        if now_ms < day_start_ms || day_end_ms <= now_ms {
            return Err(AppError::InvalidTimeRange(
                "usage limit snooze must be scheduled during its local day".to_owned(),
            ));
        }
        let snoozed_until_ms = now_ms
            .checked_add(minutes.saturating_mul(60_000))
            .ok_or_else(|| {
                AppError::InvalidTimeRange("usage limit snooze overflows time".to_owned())
            })?;
        if snoozed_until_ms > day_end_ms {
            return Err(AppError::InvalidTimeRange(
                "usage limit notification snooze cannot cross the local day boundary".to_owned(),
            ));
        }

        let connection = self.database.lock()?;
        ensure_usage_limit_rule_exists(&connection, rule_id)?;
        connection.execute(
            "INSERT INTO usage_limit_daily_exceptions (
                rule_id, local_date, notifications_snoozed_until_ms,
                created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(rule_id, local_date) DO UPDATE SET
                notifications_snoozed_until_ms = excluded.notifications_snoozed_until_ms,
                notifications_silenced = 0,
                updated_at_ms = excluded.updated_at_ms",
            params![rule_id, local_date, snoozed_until_ms, now_ms],
        )?;
        usage_limit_daily_exception_by_rule_and_date(&connection, rule_id, local_date)
    }

    pub fn silence_usage_limit_notifications_for_today(
        &self,
        rule_id: i64,
        local_date: &str,
        now_ms: i64,
    ) -> AppResult<UsageLimitDailyException> {
        validate_usage_limit_local_date(local_date)?;
        let connection = self.database.lock()?;
        ensure_usage_limit_rule_exists(&connection, rule_id)?;
        connection.execute(
            "INSERT INTO usage_limit_daily_exceptions (
                rule_id, local_date, notifications_silenced, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, 1, ?3, ?3)
             ON CONFLICT(rule_id, local_date) DO UPDATE SET
                notifications_snoozed_until_ms = NULL,
                notifications_silenced = 1,
                updated_at_ms = excluded.updated_at_ms",
            params![rule_id, local_date, now_ms],
        )?;
        usage_limit_daily_exception_by_rule_and_date(&connection, rule_id, local_date)
    }

    pub fn add_temporary_usage_limit_minutes(
        &self,
        rule_id: i64,
        local_date: &str,
        minutes: i64,
        now_ms: i64,
    ) -> AppResult<UsageLimitDailyException> {
        if !(1..=1_440).contains(&minutes) {
            return Err(AppError::InvalidTimeRange(
                "temporary usage limit minutes must be between 1 and 1440".to_owned(),
            ));
        }
        validate_usage_limit_local_date(local_date)?;
        let connection = self.database.lock()?;
        ensure_usage_limit_rule_exists(&connection, rule_id)?;
        connection.execute(
            "INSERT INTO usage_limit_daily_exceptions (
                rule_id, local_date, temporary_added_minutes, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(rule_id, local_date) DO UPDATE SET
                temporary_added_minutes = MIN(1440, temporary_added_minutes + excluded.temporary_added_minutes),
                updated_at_ms = excluded.updated_at_ms",
            params![rule_id, local_date, minutes, now_ms],
        )?;
        usage_limit_daily_exception_by_rule_and_date(&connection, rule_id, local_date)
    }

    pub fn clear_temporary_usage_limit_minutes(
        &self,
        rule_id: i64,
        local_date: &str,
        now_ms: i64,
    ) -> AppResult<UsageLimitDailyException> {
        validate_usage_limit_local_date(local_date)?;
        let mut connection = self.database.lock()?;
        ensure_usage_limit_rule_exists(&connection, rule_id)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE usage_limit_daily_exceptions
             SET temporary_added_minutes = 0, updated_at_ms = ?3
             WHERE rule_id = ?1 AND local_date = ?2",
            params![rule_id, local_date, now_ms],
        )?;
        transaction.execute(
            "DELETE FROM usage_limit_daily_exceptions
             WHERE rule_id = ?1 AND local_date = ?2
               AND temporary_added_minutes = 0
               AND notifications_snoozed_until_ms IS NULL
               AND notifications_silenced = 0",
            params![rule_id, local_date],
        )?;
        transaction.commit()?;
        usage_limit_daily_exception_by_rule_and_date(&connection, rule_id, local_date)
    }

    pub fn usage_limit_reminder_history(
        &self,
        start_local_date: &str,
        end_local_date: &str,
    ) -> AppResult<Vec<UsageLimitReminderHistoryEntry>> {
        let start_date = validate_usage_limit_local_date(start_local_date)?;
        let end_date = validate_usage_limit_local_date(end_local_date)?;
        if end_date < start_date {
            return Err(AppError::InvalidTimeRange(
                "usage limit reminder history end date must not be before its start date"
                    .to_owned(),
            ));
        }
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(
            "SELECT rule_id, scope_type, application_id, application_name, category,
                    target_name, local_date, threshold, delivered_at_ms
             FROM usage_limit_reminder_history
             WHERE local_date >= ?1 AND local_date <= ?2
             ORDER BY delivered_at_ms DESC, id DESC",
        )?;
        statement
            .query_map(
                params![start_local_date, end_local_date],
                map_usage_limit_reminder_history,
            )?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn data_health_summary(&self) -> AppResult<DataHealthSummary> {
        let connection = self.database.lock()?;
        let ranges = closed_health_ranges(&connection)?;
        drop(connection);
        let zero_duration_session_count = ranges
            .iter()
            .filter(|range| range.started_at_ms == range.ended_at_ms)
            .count() as i64;
        let overlapping_session_count = overlapping_session_ids(ranges).len() as i64;
        Ok(DataHealthSummary {
            overlapping_session_count,
            zero_duration_session_count,
        })
    }

    pub fn repair_data_health(&self, backup_path: &str) -> AppResult<DataHealthRepairResult> {
        let mut connection = self.database.lock()?;
        let transaction = connection.transaction()?;
        let ranges = closed_health_ranges(&transaction)?;
        let mut sessions = ranges
            .iter()
            .filter(|range| range.ended_at_ms > range.started_at_ms)
            .map(|range| (range.id, range.started_at_ms, range.ended_at_ms))
            .collect::<Vec<_>>();
        sessions.sort_unstable_by_key(|(id, start, end)| (*start, *end, *id));
        let overlapping_ids = overlapping_session_ids(ranges);
        let mut snapshot_statement = transaction.prepare(CLOSED_HEALTH_SESSIONS_QUERY)?;
        let snapshot = snapshot_statement
            .query_map([], |row| {
                let id = row.get::<_, i64>(0)?;
                let duration_ms = row.get::<_, i64>(6)?;
                if duration_ms == 0 || overlapping_ids.contains(&id) {
                    Ok(Some(DataHealthUndoSession {
                        session: map_session(row)?,
                        category_override: row.get(12)?,
                    }))
                } else {
                    Ok(None)
                }
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        drop(snapshot_statement);
        transaction.execute(
            "INSERT INTO data_health_undo(singleton_id, snapshot_json, backup_path, created_at_ms)
             VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(singleton_id) DO UPDATE SET
               snapshot_json = excluded.snapshot_json,
               backup_path = excluded.backup_path,
               created_at_ms = excluded.created_at_ms",
            params![
                serde_json::to_string(&DataHealthUndoSnapshot {
                    organizations: session_organization_snapshots(
                        &transaction,
                        &snapshot
                            .iter()
                            .map(|saved| saved.session.id)
                            .collect::<Vec<_>>(),
                    )?,
                    sessions: snapshot,
                })
                .map_err(|error| AppError::InvalidSession(format!(
                    "could not create health repair snapshot: {error}"
                )))?,
                backup_path,
                now_millis(),
            ],
        )?;
        let mut deleted_session_count = transaction.execute(
            "DELETE FROM activity_sessions WHERE is_open = 0 AND duration_ms = 0",
            [],
        )?;
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
        for saved in &snapshot.sessions {
            let session = &saved.session;
            transaction.execute("DELETE FROM activity_sessions WHERE id = ?1", [session.id])?;
            transaction.execute(
                "INSERT INTO activity_sessions (
                   id, state, application_id, window_title, started_at_ms, ended_at_ms,
                   duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms, note,
                   category_override
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                    saved.category_override,
                ],
            )?;
        }
        restore_session_organization_snapshots(&transaction, &snapshot.organizations)?;
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
        validate_clock_time(&settings.weekly_report_notification_time)?;
        if !(1..=7).contains(&settings.weekly_report_notification_weekday) {
            return Err(AppError::InvalidMonitorConfiguration(
                "weekly report notification weekday must be between 1 and 7".to_owned(),
            ));
        }
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
                quiet_hours_end = ?17, automatic_encrypted_backup_enabled = ?18,
                weekly_report_auto_archive_enabled = ?19,
                weekly_report_notification_enabled = ?20,
                weekly_report_notification_weekday = ?21,
                weekly_report_notification_time = ?22, updated_at_ms = ?23
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
                settings.automatic_encrypted_backup_enabled,
                settings.weekly_report_auto_archive_enabled,
                settings.weekly_report_notification_enabled,
                settings.weekly_report_notification_weekday,
                settings.weekly_report_notification_time,
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
        transaction.execute("DELETE FROM usage_limit_reminder_history", [])?;
        transaction.execute("DELETE FROM usage_limit_alerts", [])?;
        transaction.execute("DELETE FROM usage_limit_daily_exceptions", [])?;
        transaction.execute(
            "DELETE FROM applications
             WHERE NOT EXISTS (
                SELECT 1 FROM usage_limit_rules
                WHERE application_id = applications.id
             )",
            [],
        )?;
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
                 ) AND NOT EXISTS (
                    SELECT 1 FROM usage_limit_rules
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
             ) AND NOT EXISTS (
                SELECT 1 FROM usage_limit_rules
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

    pub fn mark_encrypted_backup_completed(&self, completed_at_ms: i64) -> AppResult<()> {
        let connection = self.database.lock()?;
        connection.execute(
            "UPDATE settings SET last_encrypted_backup_at_ms = ?1 WHERE singleton_id = 1",
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
            let organizations = session_organization_snapshots(
                transaction,
                &sessions
                    .iter()
                    .map(|session| session.id)
                    .collect::<Vec<_>>(),
            )?;
            let project_ids = organizations
                .iter()
                .filter_map(|organization| organization.project_id)
                .collect::<BTreeSet<_>>();
            if project_ids.len() > 1 {
                return Err(AppError::InvalidSession(
                    "sessions assigned to different projects cannot be merged".to_owned(),
                ));
            }
            let project_id = project_ids.into_iter().next();
            let tag_ids = organizations
                .iter()
                .flat_map(|organization| organization.tag_ids.iter().copied())
                .collect::<BTreeSet<_>>();
            transaction.execute(
                "UPDATE activity_sessions SET ended_at_ms = ?2, duration_ms = ?2 - started_at_ms,
                 note = NULLIF(?3, ''), updated_at_ms = ?2 WHERE id = ?1",
                params![first.id, end, notes],
            )?;
            for session in sessions.iter().skip(1) {
                transaction.execute("DELETE FROM activity_sessions WHERE id = ?1", [session.id])?;
            }
            transaction.execute(
                "DELETE FROM session_projects WHERE session_id = ?1",
                [first.id],
            )?;
            if let Some(project_id) = project_id {
                transaction.execute(
                    "INSERT INTO session_projects (session_id, project_id) VALUES (?1, ?2)",
                    params![first.id, project_id],
                )?;
            }
            transaction.execute("DELETE FROM session_tags WHERE session_id = ?1", [first.id])?;
            for tag_id in tag_ids {
                transaction.execute(
                    "INSERT INTO session_tags (session_id, tag_id) VALUES (?1, ?2)",
                    params![first.id, tag_id],
                )?;
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
        let organizations = session_organization_snapshots(&transaction, &[session_id])?;
        let copied_organizations = organizations
            .iter()
            .map(|snapshot| SessionOrganizationSnapshot {
                session_id: new_id,
                project_id: snapshot.project_id,
                tag_ids: snapshot.tag_ids.clone(),
            })
            .collect::<Vec<_>>();
        restore_session_organization_snapshots(&transaction, &copied_organizations)?;
        let token = format!("{}-{session_id}", now_millis());
        let snapshot = TimelineUndoSnapshot {
            sessions: vec![original],
            delete_session_ids: vec![session_id, new_id],
            organizations,
            organization_only: false,
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
        self.session_edit_with_undo(session_ids, operation_label, false, operation)
    }

    fn organization_session_edit<F>(
        &self,
        session_ids: &[i64],
        operation_label: &str,
        operation: F,
    ) -> AppResult<TimelineMutationResult>
    where
        F: FnOnce(&rusqlite::Transaction<'_>, &[i64]) -> AppResult<usize>,
    {
        self.session_edit_with_undo(session_ids, operation_label, true, operation)
    }

    fn session_edit_with_undo<F>(
        &self,
        session_ids: &[i64],
        operation_label: &str,
        organization_only: bool,
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
        let snapshot = closed_session_snapshots(&transaction, &ids)?;
        let organizations = session_organization_snapshots(&transaction, &ids)?;
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
                    organizations,
                    organization_only,
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
                        organizations: Vec::new(),
                        organization_only: false,
                        operation_label: None,
                    }
                })
            })
            .map_err(|error| AppError::InvalidSession(format!("invalid undo snapshot: {error}")))?;
        let affected_count = if snapshot.organization_only {
            // A later deletion can leave an organization-only undo snapshot pointing at a
            // missing session. IDs can also be reused by SQLite, so require the original
            // creation timestamp before changing the current record's organization.
            let matching_session_ids =
                session_ids_matching_snapshots(&transaction, &snapshot.sessions)?;
            for session_id in &matching_session_ids {
                transaction.execute(
                    "DELETE FROM session_projects WHERE session_id = ?1",
                    [session_id],
                )?;
                transaction.execute(
                    "DELETE FROM session_tags WHERE session_id = ?1",
                    [session_id],
                )?;
            }
            let matching_organizations = snapshot
                .organizations
                .iter()
                .filter(|organization| matching_session_ids.contains(&organization.session_id))
                .cloned()
                .collect::<Vec<_>>();
            restore_session_organization_snapshots(&transaction, &matching_organizations)?;
            matching_session_ids.len()
        } else {
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
            restore_session_organization_snapshots(&transaction, &snapshot.organizations)?;
            snapshot.sessions.len()
        };
        transaction.execute("DELETE FROM timeline_undo WHERE token = ?1", [token])?;
        transaction.commit()?;
        Ok(affected_count)
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
                                organizations: Vec::new(),
                                organization_only: false,
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

    pub fn database_integrity_ok(&self) -> AppResult<bool> {
        let connection = self.database.lock()?;
        let result: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        Ok(result == "ok")
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
                duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms,
                category_override
             ) VALUES (?1, ?2, ?3, ?4, ?4, 0, 1, NULL, ?4, ?4, ?5)",
            params![
                session.state.as_db_str(),
                session.application_id,
                session.window_title,
                session.started_at_ms,
                session.category_override,
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
                duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms,
                category_override
             ) VALUES (?1, ?2, ?3, ?4, ?4, 0, 1, NULL, ?4, ?4, ?5)",
            params![
                next_session.state.as_db_str(),
                next_session.application_id,
                next_session.window_title,
                next_session.started_at_ms,
                next_session.category_override,
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
                a.first_seen_at_ms, a.last_seen_at_ms,
                COALESCE(s.category_override, a.category)
             FROM activity_sessions s
             LEFT JOIN applications a ON a.id = s.application_id
             WHERE s.started_at_ms < ?2 AND s.ended_at_ms > ?1
             ORDER BY s.started_at_ms, s.id",
        )?;
        let records = statement
            .query_map(params![range_start_ms, range_end_ms], map_activity_record)?
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
        self.records_overlapping_page_filtered_ordered(
            range_start_ms,
            range_end_ms,
            offset,
            limit,
            search,
            false,
        )
    }

    pub fn records_overlapping_page_filtered_descending(
        &self,
        range_start_ms: i64,
        range_end_ms: i64,
        offset: usize,
        limit: usize,
        search: &TimelineSearch,
    ) -> AppResult<Vec<ActivityRecord>> {
        self.records_overlapping_page_filtered_ordered(
            range_start_ms,
            range_end_ms,
            offset,
            limit,
            search,
            true,
        )
    }

    fn records_overlapping_page_filtered_ordered(
        &self,
        range_start_ms: i64,
        range_end_ms: i64,
        offset: usize,
        limit: usize,
        search: &TimelineSearch,
        descending: bool,
    ) -> AppResult<Vec<ActivityRecord>> {
        if range_end_ms <= range_start_ms || !(1..=1_000).contains(&limit) {
            return Err(AppError::InvalidTimeRange(
                "invalid timeline page range or limit".to_owned(),
            ));
        }
        let parameters = TimelineSearchParameters::new(search)?;
        let order = if descending { "DESC" } else { "ASC" };
        let connection = self.database.lock()?;
        let mut statement = connection.prepare(&format!(
            "SELECT
                s.id, s.state, s.application_id, s.window_title,
                s.started_at_ms, s.ended_at_ms, s.duration_ms, s.is_open,
                s.closed_reason, s.created_at_ms, s.updated_at_ms, s.note,
                a.id, a.identity_key, a.name, a.bundle_id, a.executable_path,
                a.category, a.is_ignored, a.record_window_titles,
                a.first_seen_at_ms, a.last_seen_at_ms,
                COALESCE(s.category_override, a.category)
             FROM activity_sessions s
             LEFT JOIN applications a ON a.id = s.application_id
             WHERE {TIMELINE_SEARCH_WHERE}
             ORDER BY s.started_at_ms {order}, s.id {order}
             LIMIT ?12 OFFSET ?13"
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
                    parameters.project_id,
                    parameters.tag_id,
                    parameters.unassigned_only,
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
                    parameters.project_id,
                    parameters.tag_id,
                    parameters.unassigned_only,
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
      OR COALESCE(s.category_override, a.category, '') LIKE ?8 ESCAPE '\'
      OR COALESCE(s.window_title, '') LIKE ?8 ESCAPE '\'
      OR COALESCE(s.note, '') LIKE ?8 ESCAPE '\'
      OR EXISTS (
        SELECT 1 FROM session_projects search_sp
        JOIN projects search_p ON search_p.id = search_sp.project_id
        WHERE search_sp.session_id = s.id AND search_p.name LIKE ?8 ESCAPE '\'
      )
      OR EXISTS (
        SELECT 1 FROM session_tags search_st
        JOIN activity_tags search_t ON search_t.id = search_st.tag_id
        WHERE search_st.session_id = s.id AND search_t.name LIKE ?8 ESCAPE '\'
      )
    )
    AND (
      ?9 IS NULL OR EXISTS (
        SELECT 1 FROM session_projects filter_sp
        WHERE filter_sp.session_id = s.id AND filter_sp.project_id = ?9
      )
    )
    AND (
      ?10 IS NULL OR EXISTS (
        SELECT 1 FROM session_tags filter_st
        WHERE filter_st.session_id = s.id AND filter_st.tag_id = ?10
      )
    )
    AND (
      ?11 = 0 OR (
        NOT EXISTS (SELECT 1 FROM session_projects unassigned_sp WHERE unassigned_sp.session_id = s.id)
        AND NOT EXISTS (SELECT 1 FROM session_tags unassigned_st WHERE unassigned_st.session_id = s.id)
      )
    )
"#;

struct TimelineSearchParameters {
    state: Option<&'static str>,
    minimum_duration_ms: Option<i64>,
    maximum_duration_ms: Option<i64>,
    time_from_minutes: Option<i64>,
    time_to_minutes: Option<i64>,
    query_pattern: Option<String>,
    project_id: Option<i64>,
    tag_id: Option<i64>,
    unassigned_only: bool,
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
        if [search.project_id, search.tag_id]
            .into_iter()
            .flatten()
            .any(|id| id <= 0)
        {
            return Err(AppError::InvalidTimeRange(
                "timeline organization filters must use positive identifiers".to_owned(),
            ));
        }
        if search.unassigned_only && (search.project_id.is_some() || search.tag_id.is_some()) {
            return Err(AppError::InvalidTimeRange(
                "unassigned sessions cannot be combined with project or tag filters".to_owned(),
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
        if search
            .minimum_duration_ms
            .zip(search.maximum_duration_ms)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            return Err(AppError::InvalidTimeRange(
                "timeline minimum duration cannot exceed maximum duration".to_owned(),
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
        if search
            .time_from_minutes
            .zip(search.time_to_minutes)
            .is_some_and(|(from, to)| from > to)
        {
            return Err(AppError::InvalidTimeRange(
                "timeline start time cannot be after end time".to_owned(),
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
            project_id: search.project_id,
            tag_id: search.tag_id,
            unassigned_only: search.unassigned_only,
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

fn validate_usage_limit_input(input: &UsageLimitRuleInput) -> AppResult<UsageLimitRuleInput> {
    if !(1..=1_440).contains(&input.weekday_limit_minutes)
        || !(1..=1_440).contains(&input.weekend_limit_minutes)
    {
        return Err(AppError::InvalidTimeRange(
            "usage limits must be between 1 and 1440 minutes".to_owned(),
        ));
    }
    let category = input.category.as_deref().map(str::trim).map(str::to_owned);
    let target_is_valid = match input.scope_type {
        UsageLimitScopeType::Application => input.application_id.is_some() && category.is_none(),
        UsageLimitScopeType::Category => {
            input.application_id.is_none()
                && category
                    .as_deref()
                    .is_some_and(|value| !value.is_empty() && value.chars().count() <= 40)
        }
    };
    if !target_is_valid {
        return Err(AppError::InvalidTimeRange(
            "usage limit must target exactly one application or category".to_owned(),
        ));
    }
    Ok(UsageLimitRuleInput {
        scope_type: input.scope_type,
        application_id: input.application_id,
        category,
        weekday_limit_minutes: input.weekday_limit_minutes,
        weekend_limit_minutes: input.weekend_limit_minutes,
        notifications_enabled: input.notifications_enabled,
        enabled: input.enabled,
    })
}

fn optional_text_eq_ignore_ascii_case(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        (None, None) => true,
        _ => false,
    }
}

fn validate_usage_limit_target(
    connection: &Connection,
    input: &UsageLimitRuleInput,
    excluding_id: Option<i64>,
) -> AppResult<()> {
    let duplicate = match input.scope_type {
        UsageLimitScopeType::Application => {
            let application_id = input.application_id.expect("validated application target");
            let exists = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM applications WHERE id = ?1)",
                [application_id],
                |row| row.get::<_, bool>(0),
            )?;
            if !exists {
                return Err(AppError::InvalidTimeRange(
                    "usage limit application was not found".to_owned(),
                ));
            }
            connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM usage_limit_rules
                    WHERE scope_type = 'APPLICATION' AND application_id = ?1
                      AND (?2 IS NULL OR id != ?2)
                 )",
                params![application_id, excluding_id],
                |row| row.get::<_, bool>(0),
            )?
        }
        UsageLimitScopeType::Category => connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM usage_limit_rules
                WHERE scope_type = 'CATEGORY' AND category = ?1 COLLATE NOCASE
                  AND (?2 IS NULL OR id != ?2)
             )",
            params![
                input
                    .category
                    .as_deref()
                    .expect("validated category target"),
                excluding_id
            ],
            |row| row.get::<_, bool>(0),
        )?,
    };
    if duplicate {
        return Err(AppError::InvalidTimeRange(
            "a usage limit already exists for this target".to_owned(),
        ));
    }
    Ok(())
}

fn usage_limit_rule_by_id(connection: &Connection, id: i64) -> AppResult<Option<UsageLimitRule>> {
    connection
        .query_row(
            "SELECT r.id, r.scope_type, r.application_id, a.name, r.category,
                    r.weekday_limit_minutes, r.weekend_limit_minutes,
                    r.notifications_enabled, r.enabled
             FROM usage_limit_rules r
             LEFT JOIN applications a ON a.id = r.application_id
             WHERE r.id = ?1",
            [id],
            map_usage_limit_rule,
        )
        .optional()
        .map_err(Into::into)
}

struct UsageLimitReminderTarget {
    scope_type: String,
    application_id: Option<i64>,
    application_name: Option<String>,
    category: Option<String>,
    target_name: String,
}

fn usage_limit_reminder_target_by_rule_id(
    connection: &Connection,
    rule_id: i64,
) -> AppResult<Option<UsageLimitReminderTarget>> {
    connection
        .query_row(
            "SELECT r.scope_type, r.application_id, a.name, r.category,
                    COALESCE(a.name, r.category)
             FROM usage_limit_rules r
             LEFT JOIN applications a ON a.id = r.application_id
             WHERE r.id = ?1",
            [rule_id],
            |row| {
                Ok(UsageLimitReminderTarget {
                    scope_type: row.get(0)?,
                    application_id: row.get(1)?,
                    application_name: row.get(2)?,
                    category: row.get(3)?,
                    target_name: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn ensure_usage_limit_rule_exists(connection: &Connection, rule_id: i64) -> AppResult<()> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM usage_limit_rules WHERE id = ?1)",
        [rule_id],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(AppError::InvalidTimeRange(
            "usage limit rule was not found".to_owned(),
        ))
    }
}

fn validate_usage_limit_local_date(local_date: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(local_date, "%Y-%m-%d").map_err(|_| {
        AppError::InvalidTimeRange("usage limit local date must use YYYY-MM-DD".to_owned())
    })
}

fn usage_limit_daily_exception_by_rule_and_date(
    connection: &Connection,
    rule_id: i64,
    local_date: &str,
) -> AppResult<UsageLimitDailyException> {
    connection
        .query_row(
            "SELECT rule_id, local_date, temporary_added_minutes,
                    notifications_snoozed_until_ms, notifications_silenced
             FROM usage_limit_daily_exceptions
             WHERE rule_id = ?1 AND local_date = ?2",
            params![rule_id, local_date],
            |row| {
                Ok(UsageLimitDailyException {
                    rule_id: row.get(0)?,
                    local_date: row.get(1)?,
                    temporary_added_minutes: row.get(2)?,
                    notifications_snoozed_until_ms: row.get(3)?,
                    notifications_silenced: row.get(4)?,
                })
            },
        )
        .optional()
        .map(|exception| {
            exception.unwrap_or_else(|| UsageLimitDailyException {
                rule_id,
                local_date: local_date.to_owned(),
                temporary_added_minutes: 0,
                notifications_snoozed_until_ms: None,
                notifications_silenced: false,
            })
        })
        .map_err(Into::into)
}

fn map_usage_limit_reminder_history(
    row: &Row<'_>,
) -> rusqlite::Result<UsageLimitReminderHistoryEntry> {
    let scope_type = match row.get::<_, String>(1)?.as_str() {
        "APPLICATION" => UsageLimitScopeType::Application,
        "CATEGORY" => UsageLimitScopeType::Category,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(UsageLimitReminderHistoryEntry {
        rule_id: row.get(0)?,
        scope_type,
        application_id: row.get(2)?,
        application_name: row.get(3)?,
        category: row.get(4)?,
        target_name: row.get(5)?,
        local_date: row.get(6)?,
        threshold: row.get(7)?,
        delivered_at_ms: row.get(8)?,
    })
}

fn map_usage_limit_rule(row: &Row<'_>) -> rusqlite::Result<UsageLimitRule> {
    let scope_type = match row.get::<_, String>(1)?.as_str() {
        "APPLICATION" => UsageLimitScopeType::Application,
        "CATEGORY" => UsageLimitScopeType::Category,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(UsageLimitRule {
        id: row.get(0)?,
        scope_type,
        application_id: row.get(2)?,
        application_name: row.get(3)?,
        category: row.get(4)?,
        weekday_limit_minutes: row.get(5)?,
        weekend_limit_minutes: row.get(6)?,
        notifications_enabled: row.get(7)?,
        enabled: row.get(8)?,
    })
}

fn map_category_rule(row: &Row<'_>) -> rusqlite::Result<CategoryRule> {
    Ok(CategoryRule {
        id: row.get(0)?,
        match_field: CategoryRuleMatchField::from_database_value(&row.get::<_, String>(1)?)?,
        pattern: row.get(2)?,
        category: row.get(3)?,
        priority: row.get(4)?,
        enabled: row.get(5)?,
    })
}

fn category_rules_from_connection(connection: &Connection) -> AppResult<Vec<CategoryRule>> {
    let mut statement = connection.prepare(
        "SELECT id, match_field, pattern, category, priority, enabled
         FROM category_rules ORDER BY priority, id",
    )?;
    Ok(statement
        .query_map([], map_category_rule)?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

fn find_category_rule(
    connection: &rusqlite::Connection,
    rule_id: i64,
) -> AppResult<Option<CategoryRule>> {
    connection
        .query_row(
            "SELECT id, match_field, pattern, category, priority, enabled
             FROM category_rules WHERE id = ?1",
            [rule_id],
            map_category_rule,
        )
        .optional()
        .map_err(Into::into)
}

fn enabled_category_rules(connection: &Connection) -> AppResult<Vec<CategoryRule>> {
    let mut statement = connection.prepare(
        "SELECT id, match_field, pattern, category, priority, enabled
         FROM category_rules WHERE enabled = 1 ORDER BY priority, id",
    )?;
    Ok(statement
        .query_map([], map_category_rule)?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

impl CategoryRulesReapplyChange {
    fn previous_category(&self) -> &str {
        self.previous_category_override
            .as_deref()
            .unwrap_or(&self.application_category)
    }

    fn next_category(&self) -> &str {
        self.next_category_override
            .as_deref()
            .unwrap_or(&self.application_category)
    }
}

impl NormalizedCategoryCandidates {
    fn new(application_name: &str, bundle_id: Option<&str>, window_title: Option<&str>) -> Self {
        Self {
            application_name: application_name.to_lowercase(),
            bundle_id: bundle_id.map(str::to_lowercase),
            window_title: window_title.map(str::to_lowercase),
        }
    }

    fn value(&self, match_field: CategoryRuleMatchField) -> Option<&str> {
        match match_field {
            CategoryRuleMatchField::ApplicationName => Some(&self.application_name),
            CategoryRuleMatchField::BundleId => self.bundle_id.as_deref(),
            CategoryRuleMatchField::WindowTitle => self.window_title.as_deref(),
        }
    }
}

impl PreparedCategoryRule<'_> {
    fn matches(&self, candidates: &NormalizedCategoryCandidates) -> bool {
        self.rule.enabled
            && category_rule_matches_normalized(
                self.rule.match_field,
                &self.normalized_pattern,
                candidates,
            )
    }
}

fn prepare_category_rules(rules: &[CategoryRule]) -> Vec<PreparedCategoryRule<'_>> {
    rules
        .iter()
        .map(|rule| PreparedCategoryRule {
            rule,
            normalized_pattern: rule.pattern.to_lowercase(),
        })
        .collect()
}

fn category_rule_sessions(connection: &Connection) -> AppResult<Vec<CategoryRuleSession>> {
    let mut statement = connection.prepare(
        "SELECT s.id, a.id, a.name, a.bundle_id, s.window_title, a.category,
                s.category_override
         FROM activity_sessions s
         JOIN applications a ON a.id = s.application_id
         WHERE s.state = 'ACTIVE'
         ORDER BY s.started_at_ms DESC, s.id DESC",
    )?;
    Ok(statement
        .query_map([], |row| {
            Ok(CategoryRuleSession {
                session_id: row.get(0)?,
                application_id: row.get(1)?,
                application_name: row.get(2)?,
                bundle_id: row.get(3)?,
                window_title: row.get(4)?,
                application_category: row.get(5)?,
                category_override: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

fn category_rules_reapply_changes(
    sessions: Vec<CategoryRuleSession>,
    rules: &[CategoryRule],
) -> (usize, Vec<CategoryRulesReapplyChange>) {
    let scanned_session_count = sessions.len();
    let prepared_rules = prepare_category_rules(rules);
    let mut changes = Vec::new();
    for session in sessions {
        let candidates = NormalizedCategoryCandidates::new(
            &session.application_name,
            session.bundle_id.as_deref(),
            session.window_title.as_deref(),
        );
        let next_category_override =
            resolve_category_from_prepared_rules(&prepared_rules, &candidates);
        if next_category_override != session.category_override {
            changes.push(CategoryRulesReapplyChange {
                session_id: session.session_id,
                application_name: session.application_name,
                window_title: session.window_title,
                application_category: session.application_category,
                previous_category_override: session.category_override,
                next_category_override,
            });
        }
    }
    (scanned_session_count, changes)
}

fn normalize_category_rule(input: &CategoryRuleInput) -> AppResult<CategoryRuleInput> {
    let pattern = input.pattern.trim();
    let category = input.category.trim();
    if pattern.is_empty() || pattern.chars().count() > 120 {
        return Err(AppError::InvalidSession(
            "category rule pattern must contain between 1 and 120 characters".to_owned(),
        ));
    }
    if category.is_empty() || category.chars().count() > 40 {
        return Err(AppError::InvalidSession(
            "category rule category must contain between 1 and 40 characters".to_owned(),
        ));
    }
    if !(0..=9999).contains(&input.priority) {
        return Err(AppError::InvalidSession(
            "category rule priority must be between 0 and 9999".to_owned(),
        ));
    }
    Ok(CategoryRuleInput {
        match_field: input.match_field,
        pattern: pattern.to_owned(),
        category: category.to_owned(),
        priority: input.priority,
        enabled: input.enabled,
    })
}

fn resolve_category_from_rules(
    rules: &[CategoryRule],
    application_name: &str,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
) -> Option<String> {
    let prepared_rules = prepare_category_rules(rules);
    let candidates = NormalizedCategoryCandidates::new(application_name, bundle_id, window_title);
    resolve_category_from_prepared_rules(&prepared_rules, &candidates)
}

fn resolve_category_from_prepared_rules(
    rules: &[PreparedCategoryRule<'_>],
    candidates: &NormalizedCategoryCandidates,
) -> Option<String> {
    rules
        .iter()
        .find(|prepared| prepared.matches(candidates))
        .map(|prepared| prepared.rule.category.clone())
}

fn category_rule_matches_normalized(
    match_field: CategoryRuleMatchField,
    normalized_pattern: &str,
    candidates: &NormalizedCategoryCandidates,
) -> bool {
    candidates
        .value(match_field)
        .is_some_and(|candidate| candidate.contains(normalized_pattern))
}

fn truncate_preview_text(value: &str) -> String {
    const CHARACTER_LIMIT: usize = 160;
    let mut characters = value.chars();
    let truncated = characters
        .by_ref()
        .take(CHARACTER_LIMIT)
        .collect::<String>();
    if characters.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
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
        effective_category: row.get(22)?,
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

    fn closed_idle_session(
        repository: &ActivityRepository,
        started_at_ms: i64,
        ended_at_ms: i64,
    ) -> ActivitySession {
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
                started_at_ms,
            })
            .expect("session should open");
        repository
            .close_session(session.id, ended_at_ms, ClosedReason::BecameActive)
            .expect("session should close")
    }

    fn application_usage_limit(
        repository: &ActivityRepository,
        application_id: i64,
    ) -> UsageLimitRule {
        repository
            .create_usage_limit(
                &UsageLimitRuleInput {
                    scope_type: UsageLimitScopeType::Application,
                    application_id: Some(application_id),
                    category: None,
                    weekday_limit_minutes: 60,
                    weekend_limit_minutes: 90,
                    notifications_enabled: true,
                    enabled: true,
                },
                1_000,
            )
            .expect("usage limit should be created")
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
    fn category_rules_match_case_insensitively_by_priority_and_field() {
        let repository = repository();
        repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::ApplicationName,
                pattern: "intellij".to_owned(),
                category: "Development".to_owned(),
                priority: 100,
                enabled: true,
            })
            .unwrap();
        repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::BundleId,
                pattern: "JETBRAINS".to_owned(),
                category: "Focused work".to_owned(),
                priority: 10,
                enabled: true,
            })
            .unwrap();
        repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::WindowTitle,
                pattern: "private".to_owned(),
                category: "Ignored rule".to_owned(),
                priority: 0,
                enabled: false,
            })
            .unwrap();

        assert_eq!(
            repository
                .resolve_category_rule(
                    "IntelliJ IDEA",
                    Some("com.jetbrains.intellij"),
                    Some("Private project")
                )
                .unwrap()
                .as_deref(),
            Some("Focused work")
        );
        assert_eq!(
            repository
                .resolve_category_rule("Safari", None, Some("A normal page"))
                .unwrap(),
            None
        );
    }

    #[test]
    fn category_rules_reclassify_and_clear_existing_session_overrides() {
        let repository = repository();
        let app = application(&repository, 100);
        let first = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: Some("übersicht · pull request".to_owned()),
                category_override: None,
                started_at_ms: 100,
            })
            .unwrap();
        repository
            .close_session(first.id, 200, ClosedReason::AppChanged)
            .unwrap();
        let second = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: Some("Video break".to_owned()),
                category_override: Some("Old rule".to_owned()),
                started_at_ms: 200,
            })
            .unwrap();
        repository
            .close_session(second.id, 300, ClosedReason::AppChanged)
            .unwrap();

        repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::WindowTitle,
                pattern: "ÜBERSICHT".to_owned(),
                category: "Development".to_owned(),
                priority: 10,
                enabled: true,
            })
            .unwrap();
        assert_eq!(
            repository
                .resolve_category_rule(
                    "IntelliJ IDEA",
                    Some("com.jetbrains.intellij"),
                    Some("übersicht · pull request")
                )
                .unwrap()
                .as_deref(),
            Some("Development")
        );
        let result = repository.reapply_category_rules().unwrap();
        assert_eq!(result.affected_count, 2);
        assert!(result.undo_token.is_some());

        let records = repository.records_overlapping(0, 400).unwrap();
        assert_eq!(
            records[0].application.as_ref().unwrap().category,
            "Uncategorized"
        );
        assert_eq!(
            records[0].effective_category.as_deref(),
            Some("Development")
        );
        assert_eq!(
            records[1].application.as_ref().unwrap().category,
            "Uncategorized"
        );
        assert_eq!(
            records[1].effective_category.as_deref(),
            Some("Uncategorized")
        );
        assert_eq!(
            repository.reapply_category_rules().unwrap().affected_count,
            0
        );
    }

    #[test]
    fn category_rules_validate_input_lengths_and_priority() {
        let repository = repository();
        for input in [
            CategoryRuleInput {
                match_field: CategoryRuleMatchField::ApplicationName,
                pattern: "".to_owned(),
                category: "Work".to_owned(),
                priority: 100,
                enabled: true,
            },
            CategoryRuleInput {
                match_field: CategoryRuleMatchField::ApplicationName,
                pattern: "Editor".to_owned(),
                category: "".to_owned(),
                priority: 100,
                enabled: true,
            },
            CategoryRuleInput {
                match_field: CategoryRuleMatchField::ApplicationName,
                pattern: "Editor".to_owned(),
                category: "Work".to_owned(),
                priority: 10_000,
                enabled: true,
            },
        ] {
            assert!(repository.create_category_rule(&input).is_err());
        }
    }

    #[test]
    fn category_rule_preview_counts_unicode_matches_and_shadowing_with_capped_samples() {
        let repository = repository();
        let jetbrains = application(&repository, 100);
        let safari = repository
            .upsert_application(&NewApplication {
                name: "Safari".to_owned(),
                bundle_id: Some("com.apple.Safari".to_owned()),
                executable_path: Some("/Applications/Safari.app".to_owned()),
                seen_at_ms: 100,
            })
            .unwrap();
        for index in 0..6 {
            let started_at_ms = 100 + index * 10;
            let session = repository
                .create_session(&NewSession {
                    state: ActivityState::Active,
                    application_id: Some(jetbrains.id),
                    window_title: Some(format!("übersicht · project {index}")),
                    category_override: None,
                    started_at_ms,
                })
                .unwrap();
            repository
                .close_session(session.id, started_at_ms + 5, ClosedReason::AppChanged)
                .unwrap();
        }
        let long_title = format!("ÜBERSICHT · {}", "本地报告".repeat(50));
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(safari.id),
                window_title: Some(long_title),
                category_override: None,
                started_at_ms: 200,
            })
            .unwrap();
        repository
            .close_session(session.id, 205, ClosedReason::AppChanged)
            .unwrap();

        let earlier = repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::BundleId,
                pattern: "JETBRAINS".to_owned(),
                category: "Development".to_owned(),
                priority: 10,
                enabled: true,
            })
            .unwrap();
        let preview = repository
            .preview_category_rule(
                &CategoryRuleInput {
                    match_field: CategoryRuleMatchField::WindowTitle,
                    pattern: "ÜBERSICHT".to_owned(),
                    category: "Focused work".to_owned(),
                    priority: 100,
                    enabled: true,
                },
                None,
            )
            .unwrap();

        assert_eq!(preview.matched_session_count, 7);
        assert_eq!(preview.matched_application_count, 2);
        assert_eq!(preview.effective_session_count, 1);
        assert_eq!(preview.shadowed_session_count, 6);
        assert_eq!(preview.samples.len(), 5);
        assert_eq!(preview.conflicts.len(), 1);
        assert_eq!(preview.conflicts[0].rule_id, earlier.id);
        assert_eq!(preview.conflicts[0].session_count, 6);
        assert!(preview.samples[0].would_apply);
        assert!(
            preview.samples[0]
                .window_title
                .as_deref()
                .unwrap()
                .chars()
                .count()
                <= 161
        );
    }

    #[test]
    fn category_rule_preview_respects_id_order_when_editing_equal_priorities() {
        let repository = repository();
        let app = application(&repository, 100);
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                category_override: None,
                started_at_ms: 100,
            })
            .unwrap();
        repository
            .close_session(session.id, 110, ClosedReason::AppChanged)
            .unwrap();
        let first = repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::ApplicationName,
                pattern: "intellij".to_owned(),
                category: "First".to_owned(),
                priority: 100,
                enabled: true,
            })
            .unwrap();
        repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::ApplicationName,
                pattern: "idea".to_owned(),
                category: "Second".to_owned(),
                priority: 100,
                enabled: true,
            })
            .unwrap();
        let draft = CategoryRuleInput {
            match_field: CategoryRuleMatchField::ApplicationName,
            pattern: "INTELLIJ".to_owned(),
            category: "Edited".to_owned(),
            priority: 100,
            enabled: true,
        };

        let editing = repository
            .preview_category_rule(&draft, Some(first.id))
            .unwrap();
        assert_eq!(editing.effective_session_count, 1);
        assert_eq!(editing.shadowed_session_count, 0);

        let creating = repository.preview_category_rule(&draft, None).unwrap();
        assert_eq!(creating.effective_session_count, 0);
        assert_eq!(creating.shadowed_session_count, 1);
        assert_eq!(creating.conflicts[0].rule_id, first.id);
    }

    #[test]
    fn category_rules_reorder_transactionally_and_reject_invalid_id_sets() {
        let repository = repository();
        let first = repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::ApplicationName,
                pattern: "first".to_owned(),
                category: "First".to_owned(),
                priority: 100,
                enabled: true,
            })
            .unwrap();
        let second = repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::ApplicationName,
                pattern: "second".to_owned(),
                category: "Second".to_owned(),
                priority: 10,
                enabled: true,
            })
            .unwrap();
        let third = repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::ApplicationName,
                pattern: "third".to_owned(),
                category: "Third".to_owned(),
                priority: 50,
                enabled: true,
            })
            .unwrap();

        let reordered = repository
            .reorder_category_rules(&[first.id, second.id, third.id])
            .unwrap();
        assert_eq!(
            reordered.iter().map(|rule| rule.id).collect::<Vec<_>>(),
            vec![first.id, second.id, third.id]
        );
        assert_eq!(
            reordered
                .iter()
                .map(|rule| rule.priority)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );

        assert!(
            repository
                .reorder_category_rules(&[first.id, first.id, third.id])
                .is_err()
        );
        assert!(
            repository
                .reorder_category_rules(&[first.id, second.id])
                .is_err()
        );
        assert_eq!(repository.category_rules().unwrap(), reordered);
    }

    #[test]
    fn category_rules_reapply_preview_and_undo_preserve_later_changes() {
        let repository = repository();
        let app = application(&repository, 100);
        let first = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: Some("übersicht · code".to_owned()),
                category_override: None,
                started_at_ms: 100,
            })
            .unwrap();
        repository
            .close_session(first.id, 150, ClosedReason::AppChanged)
            .unwrap();
        let second = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: Some("Break".to_owned()),
                category_override: Some("Old".to_owned()),
                started_at_ms: 200,
            })
            .unwrap();
        repository
            .close_session(second.id, 250, ClosedReason::AppChanged)
            .unwrap();
        repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::WindowTitle,
                pattern: "ÜBERSICHT".to_owned(),
                category: "Development".to_owned(),
                priority: 0,
                enabled: true,
            })
            .unwrap();

        let preview = repository.preview_category_rules_reapply().unwrap();
        assert_eq!(preview.scanned_session_count, 2);
        assert_eq!(preview.affected_session_count, 2);
        assert_eq!(preview.category_change_count, 2);
        assert_eq!(preview.assigned_session_count, 1);
        assert_eq!(preview.cleared_session_count, 1);
        assert_eq!(preview.samples.len(), 2);

        let result = repository.reapply_category_rules().unwrap();
        assert_eq!(result.affected_count, preview.affected_session_count);
        let token = result.undo_token.unwrap();
        let status = repository
            .category_rules_reapply_undo_status()
            .unwrap()
            .unwrap();
        assert_eq!(status.token, token);
        assert_eq!(status.affected_count, 2);
        assert!(status.expires_at_ms > status.created_at_ms);

        repository
            .database
            .lock()
            .unwrap()
            .execute(
                "UPDATE activity_sessions SET category_override = 'Manual later' WHERE id = ?1",
                [first.id],
            )
            .unwrap();
        assert_eq!(repository.undo_category_rules_reapply(&token).unwrap(), 1);
        let records = repository.records_overlapping(0, 300).unwrap();
        assert_eq!(
            records[0].effective_category.as_deref(),
            Some("Manual later")
        );
        assert_eq!(records[1].effective_category.as_deref(), Some("Old"));
        assert!(
            repository
                .category_rules_reapply_undo_status()
                .unwrap()
                .is_none()
        );

        let expired = repository.reapply_category_rules().unwrap();
        let expired_token = expired.undo_token.unwrap();
        repository
            .database
            .lock()
            .unwrap()
            .execute(
                "UPDATE category_rule_reapply_undo SET created_at_ms = 0",
                [],
            )
            .unwrap();
        assert!(
            repository
                .category_rules_reapply_undo_status()
                .unwrap()
                .is_none()
        );
        assert!(
            repository
                .undo_category_rules_reapply(&expired_token)
                .is_err()
        );
    }

    #[test]
    fn weekly_report_archives_upsert_notify_and_delete() {
        let repository = repository();
        let input = WeeklyReportArchiveInput {
            week_start_date: "2026-07-27".to_owned(),
            week_end_date: "2026-08-02".to_owned(),
            generated_at_ms: 100,
            active_duration_ms: 7_200_000,
            idle_duration_ms: 600_000,
            previous_week_active_duration_ms: 3_600_000,
            strongest_day_date: Some("2026-07-29".to_owned()),
            peak_hour: Some(9),
            leading_category: Some("Development".to_owned()),
            focus_completion_rate: Some(80),
            payload_json: r#"{"version":1}"#.to_owned(),
        };

        let saved = repository.archive_weekly_report(&input).unwrap();
        assert_eq!(saved.week_start_date, "2026-07-27");
        assert_eq!(saved.notified_at_ms, None);

        let mut updated = input.clone();
        updated.generated_at_ms = 200;
        updated.active_duration_ms = 8_000_000;
        let saved = repository.archive_weekly_report(&updated).unwrap();
        assert_eq!(saved.generated_at_ms, 200);
        assert_eq!(saved.active_duration_ms, 8_000_000);

        repository
            .mark_weekly_report_notified("2026-07-27", 300)
            .unwrap();
        let archives = repository.weekly_report_archives(12).unwrap();
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].notified_at_ms, Some(300));

        repository
            .delete_weekly_report_archive("2026-07-27")
            .unwrap();
        assert!(repository.weekly_report_archives(12).unwrap().is_empty());
    }

    #[test]
    fn weekly_report_archive_rejects_invalid_ranges_and_payloads() {
        let repository = repository();
        let invalid = WeeklyReportArchiveInput {
            week_start_date: "2026-07-27".to_owned(),
            week_end_date: "2026-07-31".to_owned(),
            generated_at_ms: 100,
            active_duration_ms: 1,
            idle_duration_ms: 0,
            previous_week_active_duration_ms: 0,
            strongest_day_date: None,
            peak_hour: None,
            leading_category: None,
            focus_completion_rate: None,
            payload_json: "not-json".to_owned(),
        };
        assert!(repository.archive_weekly_report(&invalid).is_err());
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
    fn usage_limits_validate_targets_and_enforce_uniqueness() {
        let repository = repository();
        let app = application(&repository, 100);
        let application_rule = repository
            .create_usage_limit(
                &UsageLimitRuleInput {
                    scope_type: UsageLimitScopeType::Application,
                    application_id: Some(app.id),
                    category: None,
                    weekday_limit_minutes: 120,
                    weekend_limit_minutes: 180,
                    notifications_enabled: true,
                    enabled: true,
                },
                1_000,
            )
            .expect("application rule should be created");
        assert_eq!(
            application_rule.application_name.as_deref(),
            Some("IntelliJ IDEA")
        );
        assert!(
            repository
                .create_usage_limit(
                    &UsageLimitRuleInput {
                        scope_type: UsageLimitScopeType::Application,
                        application_id: Some(app.id),
                        category: None,
                        weekday_limit_minutes: 60,
                        weekend_limit_minutes: 60,
                        notifications_enabled: false,
                        enabled: true,
                    },
                    1_001,
                )
                .is_err()
        );

        let category_rule = repository
            .create_usage_limit(
                &UsageLimitRuleInput {
                    scope_type: UsageLimitScopeType::Category,
                    application_id: None,
                    category: Some("  Work  ".to_owned()),
                    weekday_limit_minutes: 240,
                    weekend_limit_minutes: 60,
                    notifications_enabled: true,
                    enabled: false,
                },
                1_002,
            )
            .expect("category rule should be created");
        assert_eq!(category_rule.category.as_deref(), Some("Work"));
        assert!(
            repository
                .create_usage_limit(
                    &UsageLimitRuleInput {
                        scope_type: UsageLimitScopeType::Category,
                        application_id: None,
                        category: Some("work".to_owned()),
                        weekday_limit_minutes: 60,
                        weekend_limit_minutes: 60,
                        notifications_enabled: true,
                        enabled: true,
                    },
                    1_003,
                )
                .is_err()
        );
        assert!(
            repository
                .create_usage_limit(
                    &UsageLimitRuleInput {
                        scope_type: UsageLimitScopeType::Category,
                        application_id: Some(app.id),
                        category: Some("Work".to_owned()),
                        weekday_limit_minutes: 60,
                        weekend_limit_minutes: 60,
                        notifications_enabled: true,
                        enabled: true,
                    },
                    1_004,
                )
                .is_err()
        );
        assert!(
            repository
                .update_usage_limit(
                    category_rule.id,
                    &UsageLimitRuleInput {
                        scope_type: UsageLimitScopeType::Category,
                        application_id: None,
                        category: Some("Work".to_owned()),
                        weekday_limit_minutes: 0,
                        weekend_limit_minutes: 60,
                        notifications_enabled: true,
                        enabled: true,
                    },
                    1_005,
                )
                .is_err()
        );

        repository
            .delete_usage_limit(application_rule.id)
            .expect("application rule should be deleted");
        assert_eq!(repository.usage_limit_rules().unwrap(), vec![category_rule]);
    }

    #[test]
    fn usage_limit_targets_do_not_scan_activity_sessions() {
        let repository = repository();
        let app = application(&repository, 100);
        repository
            .update_application_preferences(app.id, "Deep Work", false, false)
            .expect("category should update");
        repository
            .create_category_rule(&CategoryRuleInput {
                match_field: CategoryRuleMatchField::WindowTitle,
                pattern: "meeting".to_owned(),
                category: "Communication".to_owned(),
                priority: 100,
                enabled: true,
            })
            .expect("category rule should be stored");

        let targets = repository
            .usage_limit_targets()
            .expect("targets should load");
        assert_eq!(
            targets.applications,
            vec![UsageLimitApplicationTarget {
                application_id: app.id,
                application_name: "IntelliJ IDEA".to_owned(),
            }]
        );
        assert_eq!(targets.categories, vec!["Communication", "Deep Work"]);
    }

    #[test]
    fn usage_limit_duration_clips_active_time_and_alerts_are_deduplicated() {
        let repository = repository();
        let app = application(&repository, 100);
        repository
            .update_application_preferences(app.id, "Work", false, false)
            .expect("category should update");
        let rule = repository
            .create_usage_limit(
                &UsageLimitRuleInput {
                    scope_type: UsageLimitScopeType::Category,
                    application_id: None,
                    category: Some("work".to_owned()),
                    weekday_limit_minutes: 60,
                    weekend_limit_minutes: 60,
                    notifications_enabled: true,
                    enabled: true,
                },
                1_000,
            )
            .expect("rule should be created");
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                category_override: None,
                started_at_ms: 50,
            })
            .expect("session should open");
        repository
            .close_session(session.id, 250, ClosedReason::AppChanged)
            .expect("session should close");

        assert_eq!(
            repository
                .active_usage_duration_for_rule(&rule, 100, 200)
                .expect("usage should load"),
            100
        );
        repository
            .mark_usage_limit_alerts_delivered(rule.id, "2026-07-31", &[80, 100], 500)
            .expect("alerts should be recorded");
        repository
            .mark_usage_limit_alerts_delivered(rule.id, "2026-07-31", &[80], 600)
            .expect("duplicate alert should be ignored");
        assert_eq!(
            repository
                .delivered_usage_limit_thresholds(rule.id, "2026-07-31")
                .expect("alerts should load"),
            vec![80, 100]
        );
        repository
            .update_usage_limit(
                rule.id,
                &UsageLimitRuleInput {
                    scope_type: UsageLimitScopeType::Category,
                    application_id: None,
                    category: Some("Work".to_owned()),
                    weekday_limit_minutes: 120,
                    weekend_limit_minutes: 60,
                    notifications_enabled: true,
                    enabled: true,
                },
                700,
            )
            .expect("changed limit should update");
        assert!(
            repository
                .delivered_usage_limit_thresholds(rule.id, "2026-07-31")
                .expect("alerts should load")
                .is_empty()
        );
        assert_eq!(
            repository
                .usage_limit_reminder_history("2026-07-31", "2026-07-31")
                .expect("immutable reminder history should load")
                .len(),
            2
        );
    }

    #[test]
    fn usage_limit_daily_exceptions_validate_boundaries_and_expire_by_local_date() {
        let repository = repository();
        let app = application(&repository, 100);
        let rule = application_usage_limit(&repository, app.id);
        let day = "2026-07-31";

        assert!(
            repository
                .snooze_usage_limit_notifications(rule.id, day, 4, 100_000, 0, 1_000_000)
                .is_err()
        );
        assert!(
            repository
                .snooze_usage_limit_notifications(rule.id, day, 5, 900_000, 0, 1_000_000)
                .is_err()
        );
        let snoozed = repository
            .snooze_usage_limit_notifications(rule.id, day, 5, 100_000, 0, 1_000_000)
            .expect("snooze should be stored");
        assert_eq!(snoozed.notifications_snoozed_until_ms, Some(400_000));

        let silenced = repository
            .silence_usage_limit_notifications_for_today(rule.id, day, 110_000)
            .expect("silence should be stored");
        assert!(silenced.notifications_silenced);
        assert_eq!(silenced.notifications_snoozed_until_ms, None);
        let resumed = repository
            .snooze_usage_limit_notifications(rule.id, day, 5, 120_000, 0, 1_000_000)
            .expect("snoozing should resume notifications after the delay");
        assert!(!resumed.notifications_silenced);

        repository
            .add_temporary_usage_limit_minutes(rule.id, day, 1_000, 130_000)
            .expect("temporary limit should be stored");
        let capped = repository
            .add_temporary_usage_limit_minutes(rule.id, day, 1_000, 140_000)
            .expect("temporary limit should cap instead of overflowing");
        assert_eq!(capped.temporary_added_minutes, 1_440);
        assert!(
            repository
                .add_temporary_usage_limit_minutes(rule.id, day, 0, 150_000)
                .is_err()
        );
        assert_eq!(
            repository
                .usage_limit_daily_exception(rule.id, "2026-08-01")
                .expect("the following date should load a fresh exception")
                .temporary_added_minutes,
            0
        );
        let cleared = repository
            .clear_temporary_usage_limit_minutes(rule.id, day, 160_000)
            .expect("temporary limit should be cleared");
        assert_eq!(cleared.temporary_added_minutes, 0);
        assert_eq!(cleared.notifications_snoozed_until_ms, Some(420_000));
    }

    #[test]
    fn usage_limit_exception_constraints_and_application_cascade_are_enforced() {
        let repository = repository();
        let app = application(&repository, 100);
        let rule = application_usage_limit(&repository, app.id);
        repository
            .add_temporary_usage_limit_minutes(rule.id, "2026-07-31", 30, 100)
            .expect("exception should be stored");
        let connection = repository.database.lock().expect("database should lock");
        assert!(
            connection
                .execute(
                    "INSERT INTO usage_limit_daily_exceptions (
                    rule_id, local_date, temporary_added_minutes, created_at_ms, updated_at_ms
                 ) VALUES (?1, 'bad-date', 0, 1, 1)",
                    [rule.id],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO usage_limit_daily_exceptions (
                    rule_id, local_date, temporary_added_minutes, created_at_ms, updated_at_ms
                 ) VALUES (?1, '2026-08-01', 1441, 1, 1)",
                    [rule.id],
                )
                .is_err()
        );
        connection
            .execute("DELETE FROM applications WHERE id = ?1", [app.id])
            .expect("deleting an application should cascade through its usage rule");
        let exceptions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM usage_limit_daily_exceptions WHERE rule_id = ?1",
                [rule.id],
                |row| row.get(0),
            )
            .expect("exception count should load");
        assert_eq!(exceptions, 0);
    }

    #[test]
    fn reminder_history_uses_delivery_snapshots_and_orders_newest_first() {
        let repository = repository();
        let first_app = application(&repository, 100);
        let second_app = repository
            .upsert_application(&NewApplication {
                name: "Browser".to_owned(),
                bundle_id: Some("example.browser".to_owned()),
                executable_path: None,
                seen_at_ms: 100,
            })
            .expect("second application should be stored");
        let first_rule = application_usage_limit(&repository, first_app.id);
        let second_rule = application_usage_limit(&repository, second_app.id);
        repository
            .mark_usage_limit_alerts_delivered(first_rule.id, "2026-07-30", &[80], 100)
            .expect("first alert should be recorded");
        repository
            .mark_usage_limit_alerts_delivered(second_rule.id, "2026-07-31", &[100], 300)
            .expect("second alert should be recorded");

        repository
            .update_usage_limit(
                first_rule.id,
                &UsageLimitRuleInput {
                    scope_type: UsageLimitScopeType::Application,
                    application_id: Some(first_app.id),
                    category: None,
                    weekday_limit_minutes: 120,
                    weekend_limit_minutes: 90,
                    notifications_enabled: true,
                    enabled: true,
                },
                400,
            )
            .expect("updating a rule should reset only its dedupe record");
        repository
            .delete_usage_limit(second_rule.id)
            .expect("the second rule should be deleted");

        let history = repository
            .usage_limit_reminder_history("2026-07-30", "2026-07-31")
            .expect("history should load");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].target_name, "Browser");
        assert_eq!(history[0].threshold, 100);
        assert_eq!(history[1].target_name, "IntelliJ IDEA");
        assert_eq!(history[1].threshold, 80);
    }

    #[test]
    fn delete_all_activity_clears_usage_limit_reminders_and_today_exceptions() {
        let repository = repository();
        let app = application(&repository, 100);
        let rule = application_usage_limit(&repository, app.id);
        repository
            .mark_usage_limit_alerts_delivered(rule.id, "2026-07-31", &[80], 100)
            .expect("alert should be recorded");
        repository
            .add_temporary_usage_limit_minutes(rule.id, "2026-07-31", 30, 100)
            .expect("today exception should be recorded");

        repository
            .delete_all_activity()
            .expect("all activity data should be deleted");

        assert!(
            repository
                .delivered_usage_limit_thresholds(rule.id, "2026-07-31")
                .expect("dedupe records should load")
                .is_empty()
        );
        assert!(
            repository
                .usage_limit_reminder_history("2026-07-31", "2026-07-31")
                .expect("history should load")
                .is_empty()
        );
        assert_eq!(
            repository
                .usage_limit_daily_exception(rule.id, "2026-07-31")
                .expect("exception should load")
                .temporary_added_minutes,
            0
        );
        assert_eq!(repository.usage_limit_rules().unwrap().len(), 1);
    }

    fn health_ranges(intervals: &[(i64, i64)]) -> Vec<HealthSessionRange> {
        intervals
            .iter()
            .enumerate()
            .map(|(index, (started_at_ms, ended_at_ms))| HealthSessionRange {
                id: index as i64 + 1,
                started_at_ms: *started_at_ms,
                ended_at_ms: *ended_at_ms,
            })
            .collect()
    }

    #[test]
    fn data_health_overlap_detection_marks_every_nested_participant() {
        assert_eq!(
            overlapping_session_ids(health_ranges(&[(0, 100), (10, 20), (30, 40)])),
            HashSet::from([1, 2, 3])
        );
    }

    #[test]
    fn data_health_overlap_detection_handles_equal_starts_without_marking_empty_ranges() {
        assert_eq!(
            overlapping_session_ids(health_ranges(&[(10, 50), (10, 30), (10, 10)])),
            HashSet::from([1, 2])
        );
    }

    #[test]
    fn data_health_overlap_detection_marks_every_chained_participant() {
        assert_eq!(
            overlapping_session_ids(health_ranges(&[(0, 10), (5, 15), (14, 20)])),
            HashSet::from([1, 2, 3])
        );
    }

    #[test]
    fn data_health_overlap_detection_keeps_touching_ranges_separate() {
        assert!(overlapping_session_ids(health_ranges(&[(0, 10), (10, 20), (20, 30)])).is_empty());
    }

    #[test]
    fn data_health_overlap_detection_preserves_interior_zero_duration_behavior() {
        assert_eq!(
            overlapping_session_ids(health_ranges(&[(0, 100), (50, 50)])),
            HashSet::from([1, 2])
        );
    }

    #[test]
    fn data_health_summary_ignores_open_sessions() {
        let repository = repository();
        {
            let connection = repository.database.lock().unwrap();
            connection
                .execute(
                    "INSERT INTO activity_sessions (
                       state, application_id, started_at_ms, ended_at_ms, duration_ms,
                       is_open, closed_reason, created_at_ms, updated_at_ms
                     ) VALUES ('IDLE', NULL, 0, 100, 100, 0, 'BECAME_ACTIVE', 0, 100)",
                    [],
                )
                .unwrap();
        }
        repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
                started_at_ms: 50,
            })
            .expect("open session should be created");

        assert_eq!(
            repository.data_health_summary().unwrap(),
            DataHealthSummary {
                overlapping_session_count: 0,
                zero_duration_session_count: 0,
            }
        );
    }

    #[test]
    fn data_health_overlap_detection_scales_to_large_histories() {
        let mut ranges = (0..50_000_i64)
            .map(|index| HealthSessionRange {
                id: index + 1,
                started_at_ms: index * 10,
                ended_at_ms: index * 10 + 10,
            })
            .collect::<Vec<_>>();
        ranges.push(HealthSessionRange {
            id: 50_001,
            started_at_ms: 15,
            ended_at_ms: 16,
        });

        assert_eq!(overlapping_session_ids(ranges), HashSet::from([2, 50_001]));
    }

    #[test]
    fn data_health_queries_do_not_use_correlated_subqueries() {
        let repository = repository();
        let connection = repository.database.lock().unwrap();

        for query in [CLOSED_HEALTH_RANGES_QUERY, CLOSED_HEALTH_SESSIONS_QUERY] {
            let details = connection
                .prepare(&format!("EXPLAIN QUERY PLAN {query}"))
                .expect("health query plan should prepare")
                .query_map([], |row| row.get::<_, String>(3))
                .expect("health query plan should run")
                .collect::<Result<Vec<_>, _>>()
                .expect("health query plan should decode");
            assert!(
                details
                    .iter()
                    .all(|detail| !detail.to_ascii_uppercase().contains("CORRELATED")),
                "health query must not use a correlated subquery: {details:?}"
            );
        }
    }

    #[test]
    fn data_health_repair_trims_overlaps_and_removes_zero_duration_sessions() {
        let repository = repository();
        let project = repository
            .create_project(&ProjectInput {
                name: "Health repair".to_owned(),
                color: "#123456".to_owned(),
            })
            .unwrap();
        let tag = repository
            .create_activity_tag(&ActivityTagInput {
                name: "Preserved".to_owned(),
                color: "#654321".to_owned(),
            })
            .unwrap();
        let session_ids = {
            let connection = repository.database.lock().unwrap();
            let mut session_ids = Vec::new();
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
                session_ids.push(connection.last_insert_rowid());
            }
            connection
                .execute(
                    "UPDATE activity_sessions SET category_override = 'Manual review'
                     WHERE started_at_ms = 100",
                    [],
                )
                .unwrap();
            session_ids
        };
        for session_id in &session_ids {
            repository
                .set_session_organization(*session_id, Some(project.id), &[tag.id])
                .unwrap();
        }
        let summary = repository.data_health_summary().unwrap();
        assert_eq!(summary.overlapping_session_count, 2);
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
        let restored_category: Option<String> = repository
            .database
            .lock()
            .unwrap()
            .query_row(
                "SELECT category_override FROM activity_sessions WHERE started_at_ms = 100",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(restored_category.as_deref(), Some("Manual review"));
        for session_id in session_ids {
            let organization = repository.get_session_organization(session_id).unwrap();
            assert_eq!(
                organization.project.as_ref().map(|item| item.id),
                Some(project.id)
            );
            assert_eq!(organization.tags, vec![tag.clone()]);
        }
        assert!(!repository.data_health_undo_status().unwrap().available);
    }

    #[test]
    fn data_health_repair_snapshots_every_nested_overlap_participant() {
        let repository = repository();
        {
            let connection = repository.database.lock().unwrap();
            for (start, end) in [(0, 100), (10, 20), (30, 40)] {
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

        assert_eq!(
            repository
                .data_health_summary()
                .unwrap()
                .overlapping_session_count,
            3
        );
        let repaired = repository
            .repair_data_health("/tmp/watchhouse-nested-health.sqlite3")
            .unwrap();
        assert_eq!(repaired.trimmed_session_count, 1);
        assert_eq!(repaired.deleted_session_count, 0);
        assert_eq!(
            repository
                .data_health_summary()
                .unwrap()
                .overlapping_session_count,
            0
        );
        assert_eq!(repository.undo_data_health_repair().unwrap(), 3);
        assert_eq!(
            repository
                .data_health_summary()
                .unwrap()
                .overlapping_session_count,
            3
        );
    }

    #[test]
    fn active_session_requires_an_application() {
        let error = repository()
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: None,
                window_title: None,
                category_override: None,
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
                category_override: None,
                started_at_ms: 100,
            })
            .expect("first session should open");

        let error = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
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
                category_override: None,
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
                category_override: None,
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
        repository
            .create_usage_limit(
                &UsageLimitRuleInput {
                    scope_type: UsageLimitScopeType::Application,
                    application_id: Some(app.id),
                    category: None,
                    weekday_limit_minutes: 60,
                    weekend_limit_minutes: 90,
                    notifications_enabled: true,
                    enabled: true,
                },
                100,
            )
            .expect("usage limit should be stored");
        let old = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(app.id),
                window_title: None,
                category_override: None,
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
                category_override: None,
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
        assert!(!result.deleted_application_ids.contains(&app.id));
        assert!(
            repository
                .application(app.id)
                .expect("limited application should load")
                .is_some()
        );
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
                category_override: None,
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
                    category_override: None,
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
    fn timeline_search_defaults_fields_missing_from_older_ipc_payloads() {
        let search: TimelineSearch = serde_json::from_str("{}").unwrap();
        assert_eq!(search, TimelineSearch::default());
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
                category_override: None,
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
                category_override: None,
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
                category_override: None,
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
    fn timeline_search_range_clips_boundary_sessions_and_paginates() {
        let repository = repository();
        let app = application(&repository, 0);
        let day_ms = 24 * 60 * 60_000_i64;
        let range_start = day_ms;
        let range_end = 3 * day_ms;

        for (start, end) in [
            (range_start - 100, range_start + 100),
            (range_end - 100, range_end + 100),
            (range_end + 200, range_end + 300),
        ] {
            let session = repository
                .create_session(&NewSession {
                    state: ActivityState::Active,
                    application_id: Some(app.id),
                    window_title: None,
                    category_override: None,
                    started_at_ms: start,
                })
                .expect("session should open");
            repository
                .close_session(session.id, end, ClosedReason::AppChanged)
                .expect("session should close");
        }
        let search = TimelineSearch {
            query: Some("idea".to_owned()),
            ..TimelineSearch::default()
        };

        assert_eq!(
            repository
                .timeline_page_totals_filtered(range_start, range_end, &search)
                .expect("range totals should load"),
            (2, 200, 0)
        );
        let first_page = repository
            .records_overlapping_page_filtered(range_start, range_end, 0, 1, &search)
            .expect("first page should load");
        let second_page = repository
            .records_overlapping_page_filtered(range_start, range_end, 1, 1, &search)
            .expect("second page should load");
        let newest_page = repository
            .records_overlapping_page_filtered_descending(range_start, range_end, 0, 1, &search)
            .expect("newest page should load");
        assert_eq!(first_page.len(), 1);
        assert_eq!(second_page.len(), 1);
        assert!(first_page[0].session.id < second_page[0].session.id);
        assert_eq!(newest_page[0].session.id, second_page[0].session.id);
    }

    #[test]
    fn timeline_search_rejects_reversed_filter_bounds() {
        let repository = repository();
        let searches = [
            TimelineSearch {
                minimum_duration_ms: Some(2),
                maximum_duration_ms: Some(1),
                ..TimelineSearch::default()
            },
            TimelineSearch {
                time_from_minutes: Some(60),
                time_to_minutes: Some(30),
                ..TimelineSearch::default()
            },
        ];
        for search in searches {
            assert!(matches!(
                repository.timeline_page_totals_filtered(0, 1, &search),
                Err(AppError::InvalidTimeRange(_))
            ));
        }
    }

    #[test]
    fn recovery_closes_at_last_durable_checkpoint() {
        let repository = repository();
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
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
                category_override: None,
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
                category_override: None,
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
                category_override: None,
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
                category_override: None,
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
        let project = repository
            .create_project(&ProjectInput {
                name: "Restored".to_owned(),
                color: "#123456".to_owned(),
            })
            .unwrap();
        let tag = repository
            .create_activity_tag(&ActivityTagInput {
                name: "Undo".to_owned(),
                color: "#654321".to_owned(),
            })
            .unwrap();
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
                started_at_ms: 100,
            })
            .unwrap();
        repository
            .close_session(session.id, 200, ClosedReason::BecameActive)
            .unwrap();
        repository
            .set_session_organization(session.id, Some(project.id), &[tag.id])
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
        let organization = repository.get_session_organization(session.id).unwrap();
        assert_eq!(organization.project.unwrap(), project);
        assert_eq!(organization.tags, vec![tag]);
    }

    #[test]
    fn organization_undo_skips_deleted_or_reused_session_ids() {
        let repository = repository();
        let original_project = repository
            .create_project(&ProjectInput {
                name: "Original".to_owned(),
                color: "#123456".to_owned(),
            })
            .unwrap();
        let replacement_project = repository
            .create_project(&ProjectInput {
                name: "Replacement".to_owned(),
                color: "#654321".to_owned(),
            })
            .unwrap();
        let first = closed_idle_session(&repository, 100, 200);
        let deleted = closed_idle_session(&repository, 300, 400);
        repository
            .set_sessions_organization(&[first.id, deleted.id], Some(replacement_project.id), &[])
            .unwrap();
        let undo_token = repository
            .set_sessions_organization(&[first.id, deleted.id], Some(original_project.id), &[])
            .unwrap()
            .undo_token
            .unwrap();

        repository.delete_closed_sessions(&[deleted.id]).unwrap();
        let replacement_created_at_ms = deleted.created_at_ms.saturating_add(1);
        repository
            .database
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO activity_sessions (
                    id, state, application_id, window_title, started_at_ms, ended_at_ms,
                    duration_ms, is_open, closed_reason, created_at_ms, updated_at_ms, note
                 ) VALUES (?1, 'IDLE', NULL, NULL, 500, 600, 100, 0, 'BECAME_ACTIVE', ?2, ?2, NULL)",
                params![deleted.id, replacement_created_at_ms],
            )
            .unwrap();

        assert_eq!(repository.undo_timeline_edit(&undo_token).unwrap(), 1);
        assert_eq!(
            repository
                .get_session_organization(first.id)
                .unwrap()
                .project
                .unwrap(),
            replacement_project
        );
        let reused = repository.get_session_organization(deleted.id).unwrap();
        assert!(reused.project.is_none());
        assert!(reused.tags.is_empty());
        assert!(
            repository
                .timeline_undo_history()
                .unwrap()
                .iter()
                .all(|entry| entry.token != undo_token)
        );
    }

    #[test]
    fn merge_unions_tags_preserves_project_and_restores_each_organization_on_undo() {
        let repository = repository();
        let project = repository
            .create_project(&ProjectInput {
                name: "Merged".to_owned(),
                color: "#123456".to_owned(),
            })
            .unwrap();
        let first_tag = repository
            .create_activity_tag(&ActivityTagInput {
                name: "First".to_owned(),
                color: "#111111".to_owned(),
            })
            .unwrap();
        let second_tag = repository
            .create_activity_tag(&ActivityTagInput {
                name: "Second".to_owned(),
                color: "#222222".to_owned(),
            })
            .unwrap();
        let first = closed_idle_session(&repository, 100, 200);
        let second = closed_idle_session(&repository, 300, 400);
        repository
            .set_session_organization(first.id, None, &[first_tag.id])
            .unwrap();
        repository
            .set_session_organization(second.id, Some(project.id), &[second_tag.id])
            .unwrap();

        let result = repository
            .merge_closed_sessions(&[first.id, second.id])
            .unwrap();
        let merged = repository.get_session_organization(first.id).unwrap();
        assert_eq!(
            merged.project.as_ref().map(|item| item.id),
            Some(project.id)
        );
        assert_eq!(merged.tags, vec![first_tag.clone(), second_tag.clone()]);
        assert!(matches!(
            repository.get_session_organization(second.id),
            Err(AppError::SessionNotFound(id)) if id == second.id
        ));

        assert_eq!(
            repository
                .undo_timeline_edit(result.undo_token.as_deref().unwrap())
                .unwrap(),
            2
        );
        let restored_first = repository.get_session_organization(first.id).unwrap();
        assert!(restored_first.project.is_none());
        assert_eq!(restored_first.tags, vec![first_tag]);
        let restored_second = repository.get_session_organization(second.id).unwrap();
        assert_eq!(restored_second.project.unwrap(), project);
        assert_eq!(restored_second.tags, vec![second_tag]);
    }

    #[test]
    fn merge_rejects_sessions_from_different_projects_without_changes() {
        let repository = repository();
        let first_project = repository
            .create_project(&ProjectInput {
                name: "First".to_owned(),
                color: "#111111".to_owned(),
            })
            .unwrap();
        let second_project = repository
            .create_project(&ProjectInput {
                name: "Second".to_owned(),
                color: "#222222".to_owned(),
            })
            .unwrap();
        let first = closed_idle_session(&repository, 100, 200);
        let second = closed_idle_session(&repository, 300, 400);
        repository
            .set_session_organization(first.id, Some(first_project.id), &[])
            .unwrap();
        repository
            .set_session_organization(second.id, Some(second_project.id), &[])
            .unwrap();

        assert!(matches!(
            repository.merge_closed_sessions(&[first.id, second.id]),
            Err(AppError::InvalidSession(message))
                if message.contains("different projects")
        ));
        assert_eq!(repository.records_overlapping(0, 500).unwrap().len(), 2);
        assert_eq!(
            repository
                .get_session_organization(first.id)
                .unwrap()
                .project
                .unwrap(),
            first_project
        );
        assert_eq!(
            repository
                .get_session_organization(second.id)
                .unwrap()
                .project
                .unwrap(),
            second_project
        );
        assert!(repository.timeline_undo_history().unwrap().is_empty());
    }

    #[test]
    fn split_session_can_be_undone_without_leaving_the_new_half() {
        let repository = repository();
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
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
    fn split_and_undo_preserve_session_organization() {
        let repository = repository();
        let project = repository
            .create_project(&ProjectInput {
                name: "Project".to_owned(),
                color: "#123456".to_owned(),
            })
            .unwrap();
        let tag = repository
            .create_activity_tag(&ActivityTagInput {
                name: "Tag".to_owned(),
                color: "#654321".to_owned(),
            })
            .unwrap();
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
                started_at_ms: 100,
            })
            .unwrap();
        repository
            .close_session(session.id, 300, ClosedReason::BecameActive)
            .unwrap();
        repository
            .set_session_organization(session.id, Some(project.id), &[tag.id])
            .unwrap();

        let result = repository.split_closed_session(session.id, 200).unwrap();
        let split = repository.records_overlapping(0, 400).unwrap();
        assert_eq!(split.len(), 2);
        for record in &split {
            let organization = repository
                .get_session_organization(record.session.id)
                .unwrap();
            assert_eq!(
                organization.project.as_ref().map(|item| item.id),
                Some(project.id)
            );
            assert_eq!(
                organization
                    .tags
                    .iter()
                    .map(|item| item.id)
                    .collect::<Vec<_>>(),
                vec![tag.id]
            );
        }

        repository
            .undo_timeline_edit(result.undo_token.as_deref().unwrap())
            .unwrap();
        let organization = repository.get_session_organization(session.id).unwrap();
        assert_eq!(organization.project.unwrap().id, project.id);
        assert_eq!(organization.tags, vec![tag]);
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
