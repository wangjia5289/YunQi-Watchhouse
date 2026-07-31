import { invoke } from "@tauri-apps/api/core";

export type ActivityState = "ACTIVE" | "IDLE";

export interface ForegroundApplication {
  name: string;
  bundleIdentifier: string | null;
  executablePath: string | null;
}

export interface ActivitySample {
  observedAtMs: number;
  state: ActivityState;
  idleDurationMs: number;
  lastInputAtMs: number;
  foregroundApplication: ForegroundApplication | null;
}

export type MonitorStatus =
  | { status: "STARTING" }
  | { status: "PAUSED" }
  | { status: "RUNNING"; payload: ActivitySample }
  | { status: "DEGRADED"; payload: { message: string } }
  | { status: "STOPPED" };

export type PersistenceStatus =
  | { status: "RUNNING" }
  | { status: "DEGRADED"; payload: { message: string } }
  | { status: "STOPPED" };

export interface CurrentActivity {
  monitor: MonitorStatus;
  persistence: PersistenceStatus;
  paused: boolean;
}

export interface TimeRange {
  startMs: number;
  endMs: number;
}

export interface TodaySummary {
  date: string;
  range: TimeRange;
  activeDurationMs: number;
  idleDurationMs: number;
  firstActivityAtMs: number | null;
  lastActivityAtMs: number | null;
}

export interface FocusBlock {
  startedAtMs: number;
  endedAtMs: number;
  activeDurationMs: number;
  applicationSwitchCount: number;
  isOpen: boolean;
}

export interface FocusSummary {
  totalFocusDurationMs: number;
  longestFocusDurationMs: number;
  applicationSwitchCount: number;
  goalMinutes: number;
  breakRemindersEnabled: boolean;
  breakReminderMinutes: number;
  quietHoursStart: string;
  quietHoursEnd: string;
  blocks: FocusBlock[];
}

export interface FocusModeStatus {
  active: boolean;
  startedAtMs: number | null;
  plannedEndAtMs: number | null;
  paused: boolean;
  pausedAtMs: number | null;
  totalPausedMs: number;
}

export interface TimelineEntry {
  sessionId: number;
  applicationId: number | null;
  state: ActivityState;
  applicationName: string | null;
  bundleIdentifier: string | null;
  category: string | null;
  windowTitle: string | null;
  note: string | null;
  startedAtMs: number;
  endedAtMs: number;
  durationMs: number;
  isOpen: boolean;
}

export interface AppUsage {
  applicationId: number;
  applicationName: string;
  bundleIdentifier: string | null;
  category: string;
  isIgnored: boolean;
  recordWindowTitles: boolean;
  durationMs: number;
}

export interface CategoryUsage {
  category: string;
  durationMs: number;
  applicationCount: number;
}

export interface ApplicationIcon {
  mimeType: string;
  bytes: number[];
  revision: string;
}

export interface DailyUsage {
  date: string;
  activeDurationMs: number;
  idleDurationMs: number;
}

export interface ProductivityReport {
  range: TimeRange;
  activeDurationMs: number;
  idleDurationMs: number;
  previousActiveDurationMs: number;
  previousIdleDurationMs: number;
  dailyUsage: DailyUsage[];
  hourlyUsage: { hour: number; activeDurationMs: number }[];
  categoryUsage: CategoryUsage[];
}

export interface Settings {
  idleThresholdSeconds: number;
  launchAtLogin: boolean;
  startTrackingAutomatically: boolean;
  hideToTrayOnClose: boolean;
  recordWindowTitles: boolean;
  appearance: "SYSTEM" | "LIGHT" | "DARK";
  onboardingCompleted: boolean;
  retentionDays: 0 | 30 | 90 | 180 | 365;
  automaticBackupEnabled: boolean;
  backupInterval: "DAILY" | "WEEKLY";
  backupKeepCount: number;
  backupDirectory: string | null;
  lastMaintenanceAtMs: number;
  lastBackupAtMs: number;
  dailyFocusGoalMinutes: number;
  focusBlockGapMinutes: number;
  breakRemindersEnabled: boolean;
  breakReminderMinutes: 30 | 45 | 60 | 90 | 120;
  quietHoursStart: string;
  quietHoursEnd: string;
}

export interface MaintenancePreview {
  cutoffAtMs: number | null;
  expiredSessionCount: number;
}

export interface MaintenanceResult {
  deletedSessionCount: number;
  deletedApplicationIds: number[];
}

export interface MaintenanceStatus {
  running: boolean;
  lastSuccessAtMs: number | null;
  lastError: string | null;
}

export interface DiagnosticsSummary {
  applicationVersion: string;
  databasePath: string;
  databaseBytes: number;
  walBytes: number;
  iconCacheBytes: number;
  logBytes: number;
  automaticBackupBytes: number;
  automaticBackupCount: number;
  applicationCount: number;
  sessionCount: number;
}

export function getCurrentActivity(): Promise<CurrentActivity> {
  return invoke("get_current_activity");
}

export function setTrackingPaused(paused: boolean): Promise<boolean> {
  return invoke("set_tracking_paused", { paused });
}

export function getTodaySummary(): Promise<TodaySummary> {
  return invoke("get_today_summary");
}

export function getTodayFocusSummary(): Promise<FocusSummary> {
  return invoke("get_today_focus_summary");
}

export function getFocusMode(): Promise<FocusModeStatus> {
  return invoke("get_focus_mode");
}

export function setFocusMode(active: boolean): Promise<FocusModeStatus> {
  return invoke("set_focus_mode", { active });
}

export function startFocusPlan(durationMinutes: number | null): Promise<FocusModeStatus> {
  return invoke("start_focus_plan", { durationMinutes });
}

export function setFocusPlanPaused(paused: boolean): Promise<FocusModeStatus> {
  return invoke("set_focus_plan_paused", { paused });
}

export function endFocusPlan(completed = false): Promise<FocusModeStatus> {
  return invoke("end_focus_plan", { completed });
}

export function getTimeline(date: string): Promise<TimelineEntry[]> {
  return invoke("get_timeline", { date });
}

export interface TimelineMutationResult {
  affectedCount: number;
  undoToken: string | null;
}

export interface ImportPreview {
  recordCount: number;
  conflictCount: number;
  invalidCount: number;
  startedAtMs: number | null;
  endedAtMs: number | null;
}

export interface ImportResult {
  importedCount: number;
  mergedCount: number;
  skippedCount: number;
}

export function deleteTimelineSession(sessionId: number): Promise<TimelineMutationResult> {
  return invoke("delete_timeline_session", { sessionId });
}

export function deleteTimelineSessions(sessionIds: number[]): Promise<TimelineMutationResult> {
  return invoke("delete_timeline_sessions", { sessionIds });
}

export function mergeTimelineSessions(sessionIds: number[]): Promise<TimelineMutationResult> {
  return invoke("merge_timeline_sessions", { sessionIds });
}

export function splitTimelineSession(
  sessionId: number,
  splitAtMs: number,
): Promise<TimelineMutationResult> {
  return invoke("split_timeline_session", { sessionId, splitAtMs });
}

export function updateTimelineSessionNotes(
  sessionIds: number[],
  note: string | null,
): Promise<number> {
  return invoke("update_timeline_session_notes", { sessionIds, note });
}

export function updateTimelineSessionCategories(
  sessionIds: number[],
  category: string,
): Promise<number> {
  return invoke("update_timeline_session_categories", { sessionIds, category });
}

export function undoTimelineEdit(undoToken: string): Promise<number> {
  return invoke("undo_timeline_edit", { undoToken });
}

export function getTimelineUndoTokens(): Promise<string[]> {
  return invoke("get_timeline_undo_tokens");
}

export function previewActivityImport(
  contents: string,
  format: "json" | "csv",
): Promise<ImportPreview> {
  return invoke("preview_activity_import", { contents, format });
}

export function importActivity(
  contents: string,
  format: "json" | "csv",
  conflictPolicy: "skip" | "merge",
): Promise<ImportResult> {
  return invoke("import_activity", { contents, format, conflictPolicy });
}

export function updateTimelineSession(
  sessionId: number,
  startedAtMs: number,
  endedAtMs: number,
): Promise<void> {
  return invoke("update_timeline_session", { sessionId, startedAtMs, endedAtMs });
}

export function getAppUsage(
  rangeStartMs: number,
  rangeEndMs: number,
): Promise<AppUsage[]> {
  return invoke("get_app_usage", { rangeStartMs, rangeEndMs });
}

export function getCategoryUsage(
  rangeStartMs: number,
  rangeEndMs: number,
): Promise<CategoryUsage[]> {
  return invoke("get_category_usage", { rangeStartMs, rangeEndMs });
}

export function getApplicationIcon(
  applicationId: number,
): Promise<ApplicationIcon | null> {
  return invoke("get_application_icon", { applicationId });
}

export function clearApplicationIconCache(): Promise<void> {
  return invoke("clear_application_icon_cache");
}

export interface ApplicationPreferences {
  id: number;
  category: string;
  isIgnored: boolean;
  recordWindowTitles: boolean;
}

export function updateApplicationPreferences(
  applicationId: number,
  category: string,
  isIgnored: boolean,
  recordWindowTitles: boolean,
): Promise<ApplicationPreferences> {
  return invoke("update_application_preferences", {
    applicationId,
    category,
    isIgnored,
    recordWindowTitles,
  });
}

export type AccessibilityPermission = "GRANTED" | "DENIED" | "UNSUPPORTED";

export function getAccessibilityPermission(): Promise<AccessibilityPermission> {
  return invoke("get_accessibility_permission");
}

export function getDailyUsage(
  rangeStartMs: number,
  rangeEndMs: number,
): Promise<DailyUsage[]> {
  return invoke("get_daily_usage", { rangeStartMs, rangeEndMs });
}

export function getProductivityReport(
  rangeStartMs: number,
  rangeEndMs: number,
): Promise<ProductivityReport> {
  return invoke("get_productivity_report", { rangeStartMs, rangeEndMs });
}

export function getApplicationDailyUsage(
  applicationId: number,
  rangeStartMs: number,
  rangeEndMs: number,
): Promise<DailyUsage[]> {
  return invoke("get_application_daily_usage", {
    applicationId,
    rangeStartMs,
    rangeEndMs,
  });
}

export function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export function completeOnboarding(): Promise<Settings> {
  return invoke("complete_onboarding");
}

export function updateSettings(settings: Settings): Promise<Settings> {
  return invoke("update_settings", { settings });
}

export function deleteAllActivity(): Promise<void> {
  return invoke("delete_all_activity");
}

export function exportActivity(format: "json" | "csv"): Promise<string | null> {
  return invoke("export_activity", { format });
}

export function openDataDirectory(): Promise<void> {
  return invoke("open_data_directory");
}

export function openLogDirectory(): Promise<void> {
  return invoke("open_log_directory");
}

export function getDiagnosticsSummary(): Promise<DiagnosticsSummary> {
  return invoke("get_diagnostics_summary");
}

export function backupDatabase(): Promise<string | null> {
  return invoke("backup_database");
}

export function chooseBackupDirectory(): Promise<string | null> {
  return invoke("choose_backup_directory");
}

export function openBackupDirectory(): Promise<void> {
  return invoke("open_backup_directory");
}

export function getMaintenancePreview(): Promise<MaintenancePreview> {
  return invoke("get_maintenance_preview");
}

export function getMaintenanceStatus(): Promise<MaintenanceStatus> {
  return invoke("get_maintenance_status");
}

export function runDataMaintenance(): Promise<MaintenanceResult> {
  return invoke("run_data_maintenance");
}

export function createAutomaticBackupNow(): Promise<string> {
  return invoke("create_automatic_backup_now");
}

export function restoreDatabase(): Promise<boolean> {
  return invoke("restore_database");
}

export function optimizeDatabase(): Promise<void> {
  return invoke("optimize_database");
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return "Watchhouse could not load your activity data.";
}
