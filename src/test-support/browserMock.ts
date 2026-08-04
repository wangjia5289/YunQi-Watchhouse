import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const now = new Date();
now.setHours(12, 0, 0, 0);
const dayStart = new Date(now);
dayStart.setHours(0, 0, 0, 0);
const dayEnd = new Date(dayStart);
dayEnd.setDate(dayEnd.getDate() + 1);

const settings = {
  idleThresholdSeconds: 180,
  launchAtLogin: false,
  startTrackingAutomatically: true,
  hideToTrayOnClose: true,
  recordWindowTitles: false,
  appearance: "SYSTEM",
  onboardingCompleted: true,
  retentionDays: 0,
  automaticBackupEnabled: true,
  backupInterval: "WEEKLY",
  backupKeepCount: 5,
  backupDirectory: null,
  lastMaintenanceAtMs: now.getTime() - 86_400_000,
  lastBackupAtMs: now.getTime() - 3_600_000,
  automaticEncryptedBackupEnabled: false,
  lastEncryptedBackupAtMs: 0,
  weeklyReportAutoArchiveEnabled: true,
  weeklyReportNotificationEnabled: true,
  weeklyReportNotificationWeekday: 1,
  weeklyReportNotificationTime: "09:00",
  dailyFocusGoalMinutes: 240,
  focusBlockGapMinutes: 5,
  breakRemindersEnabled: true,
  breakReminderMinutes: 60,
  quietHoursStart: "22:00",
  quietHoursEnd: "08:00",
};

let trackingPaused = false;
let archiveSaved = false;

const currentActivity = () => ({
  paused: trackingPaused,
  persistence: { status: "RUNNING" },
  monitor: trackingPaused
    ? { status: "PAUSED" }
    : {
        status: "RUNNING",
        payload: {
          observedAtMs: now.getTime(),
          state: "ACTIVE",
          idleDurationMs: 0,
          lastInputAtMs: now.getTime(),
          foregroundApplication: {
            name: "Visual Studio Code",
            bundleIdentifier: "com.microsoft.VSCode",
            executablePath: "/Applications/Visual Studio Code.app",
          },
        },
      },
});

const todaySummary = {
  date: now.toISOString().slice(0, 10),
  range: { startMs: dayStart.getTime(), endMs: dayEnd.getTime() },
  activeDurationMs: 10_500_000,
  idleDurationMs: 1_800_000,
  firstActivityAtMs: dayStart.getTime() + 32_400_000,
  lastActivityAtMs: now.getTime(),
};

const focusSummary = {
  totalFocusDurationMs: 5_400_000,
  longestFocusDurationMs: 3_000_000,
  applicationSwitchCount: 4,
  goalMinutes: 240,
  breakRemindersEnabled: true,
  breakReminderMinutes: 60,
  quietHoursStart: "22:00",
  quietHoursEnd: "08:00",
  blocks: [],
};

const focusMode = {
  active: false,
  startedAtMs: null,
  plannedEndAtMs: null,
  paused: false,
  pausedAtMs: null,
  totalPausedMs: 0,
  templateId: null,
};

const report = (startMs: number, endMs: number) => ({
  range: { startMs, endMs },
  activeDurationMs: 28_800_000,
  idleDurationMs: 4_200_000,
  previousActiveDurationMs: 25_200_000,
  previousIdleDurationMs: 4_800_000,
  dailyUsage: [
    { date: new Date(startMs).toISOString().slice(0, 10), activeDurationMs: 14_400_000, idleDurationMs: 2_100_000 },
    { date: now.toISOString().slice(0, 10), activeDurationMs: 14_400_000, idleDurationMs: 2_100_000 },
  ],
  hourlyUsage: [
    { hour: 9, activeDurationMs: 7_200_000 },
    { hour: 14, activeDurationMs: 10_800_000 },
  ],
  categoryUsage: [
    { category: "Development", durationMs: 18_000_000, applicationCount: 2 },
    { category: "Communication", durationMs: 10_800_000, applicationCount: 1 },
  ],
});

export function installBrowserMock(windowLabel: "main" | "tray-panel" = "main") {
  mockWindows(windowLabel);
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    switch (command) {
      case "get_settings": return { ...settings };
      case "update_settings": return args.settings;
      case "set_app_locale": return null;
      case "get_current_activity": return currentActivity();
      case "set_tracking_paused": trackingPaused = Boolean(args.paused); return null;
      case "get_today_summary": return todaySummary;
      case "get_today_focus_summary": return focusSummary;
      case "get_timeline": return [];
      case "get_timeline_page": return {
        entries: [],
        totalCount: 0,
        activeDurationMs: 0,
        idleDurationMs: 0,
        offset: Number(args.offset ?? 0),
        hasMore: false,
      };
      case "search_timeline_range": return {
        entries: [],
        totalCount: 0,
        activeDurationMs: 0,
        idleDurationMs: 0,
        offset: Number(args.offset ?? 0),
        hasMore: false,
      };
      case "get_timeline_undo_history": return [];
      case "get_app_usage": return [];
      case "get_category_usage": return [];
      case "get_application_daily_usage": return [];
      case "get_daily_usage": return [];
      case "get_today_usage_limit_progress": return [];
      case "get_focus_mode": return focusMode;
      case "get_focus_plan_templates": return [];
      case "get_focus_plan_history": return {
        completedCount: 3,
        cancelledCount: 1,
        totalPlannedDurationMs: 10_800_000,
        totalActualDurationMs: 9_900_000,
        totalPausedDurationMs: 300_000,
        longestCompletedStreakDays: 3,
        recentPlans: [],
      };
      case "get_productivity_report": return report(Number(args.rangeStartMs), Number(args.rangeEndMs));
      case "get_weekly_report_archives": return archiveSaved ? [{
        weekStartDate: now.toISOString().slice(0, 10),
        weekEndDate: now.toISOString().slice(0, 10),
        generatedAtMs: now.getTime(),
        activeDurationMs: 28_800_000,
        idleDurationMs: 4_200_000,
        previousWeekActiveDurationMs: 25_200_000,
        strongestDayDate: now.toISOString().slice(0, 10),
        peakHour: 14,
        leadingCategory: "Development",
        focusCompletionRate: 75,
        payloadJson: "{}",
        notifiedAtMs: null,
      }] : [];
      case "archive_weekly_report": archiveSaved = true; return { ...(args.input as object), notifiedAtMs: null };
      case "get_shortcut_settings": return { toggleFocus: null, pauseFocus: null, startTemplate: null };
      case "get_diagnostics_summary": return {
        applicationVersion: "0.1.0",
        databasePath: "/Users/demo/Library/Application Support/watchhouse.sqlite3",
        databaseBytes: 2_097_152,
        walBytes: 0,
        iconCacheBytes: 128_000,
        logBytes: 16_000,
        automaticBackupBytes: 2_097_152,
        automaticBackupCount: 3,
        applicationCount: 14,
        sessionCount: 240,
        databaseIntegrityOk: true,
        accessibilityPermission: "GRANTED",
        notificationPermission: "GRANTED",
        trackingPaused,
        automaticBackupEnabled: true,
        lastBackupAtMs: settings.lastBackupAtMs,
        backupDirectoryAvailable: true,
        logDirectoryAvailable: true,
        maintenanceLastError: null,
      };
      case "get_data_health_summary": return { overlappingSessionCount: 0, zeroDurationSessionCount: 0 };
      case "get_data_health_undo_status": return { available: false, backupPath: null, createdAtMs: null };
      case "get_maintenance_preview": return { cutoffAtMs: null, expiredSessionCount: 0 };
      case "get_maintenance_status": return { running: false, lastSuccessAtMs: null, lastError: null };
      case "get_accessibility_permission": return "GRANTED";
      case "get_notification_permission": return "GRANTED";
      case "get_category_rules": return [];
      case "reorder_category_rules": return [];
      case "preview_category_rule": return {
        matchedSessionCount: 7,
        matchedApplicationCount: 2,
        effectiveSessionCount: 6,
        shadowedSessionCount: 1,
        conflicts: [{
          ruleId: 4,
          matchField: "APPLICATION_NAME",
          pattern: "Insiders",
          category: "Work",
          priority: 10,
          sessionCount: 1,
        }],
        samples: [
          {
            applicationName: "Visual Studio Code",
            bundleId: "com.microsoft.VSCode",
            windowTitle: "Watchhouse - Reports.tsx",
            wouldApply: true,
            shadowedByRuleId: null,
            shadowedByCategory: null,
          },
          {
            applicationName: "Visual Studio Code Insiders",
            bundleId: "com.microsoft.VSCodeInsiders",
            windowTitle: "Watchhouse - CategoryRules.tsx",
            wouldApply: false,
            shadowedByRuleId: 4,
            shadowedByCategory: "Work",
          },
        ],
      };
      case "preview_category_rules_reapply": return {
        scannedSessionCount: 240,
        affectedSessionCount: 18,
        categoryChangeCount: 15,
        assignedSessionCount: 14,
        clearedSessionCount: 4,
        samples: [
          {
            applicationName: "Visual Studio Code",
            windowTitle: "Watchhouse - CategoryRules.tsx",
            previousCategory: "Uncategorized",
            nextCategory: "Development",
            previousIsOverride: false,
            nextIsOverride: true,
          },
        ],
      };
      case "reapply_category_rules": return {
        affectedCount: 18,
        undoToken: "browser-preview-token",
        undoCreatedAtMs: now.getTime(),
        undoExpiresAtMs: now.getTime() + 86_400_000,
      };
      case "get_category_rules_reapply_undo_status": return null;
      case "undo_category_rules_reapply": return 18;
      case "get_usage_limits": return [];
      case "get_usage_limit_targets": return { applications: [], categories: ["Development"] };
      case "get_usage_limit_reminder_history": return [];
      case "check_for_updates": return {
        configured: false,
        available: false,
        currentVersion: "0.1.0",
        version: null,
        notes: null,
        publishedAt: null,
      };
      default: throw new Error(`Unhandled browser mock command: ${command}`);
    }
  }, { shouldMockEvents: true });
}
