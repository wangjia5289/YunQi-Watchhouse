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

export interface TimelineEntry {
  sessionId: number;
  applicationId: number | null;
  state: ActivityState;
  applicationName: string | null;
  bundleIdentifier: string | null;
  category: string | null;
  windowTitle: string | null;
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

export interface Settings {
  idleThresholdSeconds: number;
  launchAtLogin: boolean;
  startTrackingAutomatically: boolean;
  hideToTrayOnClose: boolean;
  recordWindowTitles: boolean;
  appearance: "SYSTEM" | "LIGHT" | "DARK";
  onboardingCompleted: boolean;
}

export interface DiagnosticsSummary {
  applicationVersion: string;
  databasePath: string;
  databaseBytes: number;
  walBytes: number;
  iconCacheBytes: number;
  logBytes: number;
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

export function getTimeline(date: string): Promise<TimelineEntry[]> {
  return invoke("get_timeline", { date });
}

export function deleteTimelineSession(sessionId: number): Promise<void> {
  return invoke("delete_timeline_session", { sessionId });
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
}

export function updateApplicationPreferences(
  applicationId: number,
  category: string,
  isIgnored: boolean,
): Promise<ApplicationPreferences> {
  return invoke("update_application_preferences", {
    applicationId,
    category,
    isIgnored,
  });
}

export function getDailyUsage(
  rangeStartMs: number,
  rangeEndMs: number,
): Promise<DailyUsage[]> {
  return invoke("get_daily_usage", { rangeStartMs, rangeEndMs });
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
