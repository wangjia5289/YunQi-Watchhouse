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
let nextOrganizationId = 20;
let nextOrganizationUndoId = 1;

const projects = [
  {
    id: 1,
    name: "Client launch",
    color: "#4E7E68",
    archived: false,
    createdAtMs: now.getTime() - 86_400_000,
    updatedAtMs: now.getTime() - 86_400_000,
  },
  {
    id: 2,
    name: "Legacy migration",
    color: "#84715B",
    archived: true,
    createdAtMs: now.getTime() - 172_800_000,
    updatedAtMs: now.getTime() - 86_400_000,
  },
];

const activityTags = [
  {
    id: 10,
    name: "Deep work",
    color: "#596FC4",
    archived: false,
    createdAtMs: now.getTime() - 86_400_000,
    updatedAtMs: now.getTime() - 86_400_000,
  },
  {
    id: 11,
    name: "Review",
    color: "#B26458",
    archived: false,
    createdAtMs: now.getTime() - 86_400_000,
    updatedAtMs: now.getTime() - 86_400_000,
  },
];

type SessionOrganizationState = {
  project: (typeof projects)[number] | null;
  tags: (typeof activityTags)[number][];
};

const sessionOrganizations = new Map<number, SessionOrganizationState>();

function organizationItemInput(args: Record<string, unknown>, entity: "project" | "tag") {
  const input = args.input;
  if (
    typeof input !== "object"
    || input === null
    || !("name" in input)
    || !("color" in input)
    || typeof input.name !== "string"
    || typeof input.color !== "string"
  ) {
    throw new Error("invalid organization input");
  }
  const name = input.name.trim();
  const color = input.color.trim().toUpperCase();
  if (!name || [...name].length > 80) {
    throw new Error(`${entity} name must contain between 1 and 80 characters`);
  }
  if (!/^#[0-9A-F]{6}$/.test(color)) {
    throw new Error(`${entity} color must use #RRGGBB`);
  }
  return { name, color };
}

function asciiNoCase(value: string): string {
  return value.replace(/[A-Z]/g, (character) => character.toLowerCase());
}

function sameOrganizationName(left: string, right: string): boolean {
  return asciiNoCase(left) === asciiNoCase(right);
}

function sortedOrganizationItems<T extends { archived: boolean; name: string; id: number }>(
  items: T[],
): T[] {
  return [...items].sort((left, right) => (
    Number(left.archived) - Number(right.archived)
      || left.name.localeCompare(right.name, "en", { sensitivity: "accent" })
      || left.id - right.id
  ));
}

function sortedTags(tags: (typeof activityTags)[number][]) {
  return [...tags].sort((left, right) => (
    left.name.localeCompare(right.name, "en", { sensitivity: "accent" }) || left.id - right.id
  ));
}

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
  const organizationFlow = new URLSearchParams(window.location.search).has("organization-flow");
  const timelineEntries = organizationFlow ? [{
    sessionId: 501,
    applicationId: 1,
    state: "ACTIVE",
    applicationName: "Visual Studio Code",
    bundleIdentifier: "com.microsoft.VSCode",
    category: "Development",
    windowTitle: "Watchhouse - Timeline.tsx",
    note: null,
    project: null,
    tags: [],
    startedAtMs: dayStart.getTime() + 9 * 3_600_000,
    endedAtMs: dayStart.getTime() + 10 * 3_600_000,
    durationMs: 3_600_000,
    isOpen: false,
  }] : [];
  const undoOrganizations = new Map<string, Map<number, SessionOrganizationState>>();
  const undoHistory: Array<{
    token: string;
    createdAtMs: number;
    sessionCount: number;
    operationLabel: string;
  }> = [];
  const hydratedTimelineEntries = () => timelineEntries.map((entry) => {
    const organization = sessionOrganizations.get(entry.sessionId);
    return {
      ...entry,
      project: organization?.project ? { ...organization.project } : null,
      tags: organization?.tags.map((tag) => ({ ...tag })) ?? [],
    };
  });
  const filteredTimelineEntries = (filters: Record<string, unknown> = {}) => (
    hydratedTimelineEntries().filter((entry) => {
      if (filters.projectId != null && entry.project?.id !== Number(filters.projectId)) return false;
      if (filters.tagId != null && !entry.tags.some((tag) => tag.id === Number(filters.tagId))) return false;
      if (filters.unassignedOnly && (entry.project || entry.tags.length > 0)) return false;
      return true;
    })
  );
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
      case "get_timeline": return hydratedTimelineEntries();
      case "get_timeline_page": {
        const filtered = filteredTimelineEntries((args.filters ?? {}) as Record<string, unknown>);
        return {
        entries: Number(args.offset ?? 0) === 0 ? filtered : [],
        totalCount: filtered.length,
        activeDurationMs: filtered.reduce((total, entry) => total + entry.durationMs, 0),
        idleDurationMs: 0,
        offset: Number(args.offset ?? 0),
        hasMore: false,
        };
      }
      case "list_projects": return sortedOrganizationItems(
        projects.filter((item) => Boolean(args.includeArchived) || !item.archived),
      ).map((item) => ({ ...item }));
      case "create_project": {
        const input = organizationItemInput(args, "project");
        if (projects.some((item) => sameOrganizationName(item.name, input.name))) {
          throw new Error("a project with this name already exists");
        }
        const created = {
          id: nextOrganizationId++,
          ...input,
          archived: false,
          createdAtMs: now.getTime(),
          updatedAtMs: now.getTime(),
        };
        projects.push(created);
        return created;
      }
      case "update_project": {
        const project = projects.find((item) => item.id === Number(args.projectId));
        if (!project) throw new Error("project was not found");
        const input = organizationItemInput(args, "project");
        if (projects.some((item) => item.id !== project.id && sameOrganizationName(item.name, input.name))) {
          throw new Error("a project with this name already exists");
        }
        Object.assign(project, input, { updatedAtMs: now.getTime() });
        return { ...project };
      }
      case "set_project_archived": {
        const project = projects.find((item) => item.id === Number(args.projectId));
        if (!project) throw new Error("project was not found");
        project.archived = Boolean(args.archived);
        project.updatedAtMs = now.getTime();
        return { ...project };
      }
      case "list_activity_tags": return sortedOrganizationItems(
        activityTags.filter((item) => Boolean(args.includeArchived) || !item.archived),
      ).map((item) => ({ ...item }));
      case "create_activity_tag": {
        const input = organizationItemInput(args, "tag");
        if (activityTags.some((item) => sameOrganizationName(item.name, input.name))) {
          throw new Error("an activity tag with this name already exists");
        }
        const created = {
          id: nextOrganizationId++,
          ...input,
          archived: false,
          createdAtMs: now.getTime(),
          updatedAtMs: now.getTime(),
        };
        activityTags.push(created);
        return created;
      }
      case "update_activity_tag": {
        const tag = activityTags.find((item) => item.id === Number(args.tagId));
        if (!tag) throw new Error("activity tag was not found");
        const input = organizationItemInput(args, "tag");
        if (activityTags.some((item) => item.id !== tag.id && sameOrganizationName(item.name, input.name))) {
          throw new Error("an activity tag with this name already exists");
        }
        Object.assign(tag, input, { updatedAtMs: now.getTime() });
        return { ...tag };
      }
      case "set_activity_tag_archived": {
        const tag = activityTags.find((item) => item.id === Number(args.tagId));
        if (!tag) throw new Error("activity tag was not found");
        tag.archived = Boolean(args.archived);
        tag.updatedAtMs = now.getTime();
        return { ...tag };
      }
      case "get_session_organization": {
        const sessionId = Number(args.sessionId);
        if (!timelineEntries.some((entry) => entry.sessionId === sessionId)) {
          throw new Error("session was not found");
        }
        const organization = sessionOrganizations.get(sessionId) ?? { project: null, tags: [] };
        return {
          project: organization.project ? { ...organization.project } : null,
          tags: sortedTags(organization.tags).map((tag) => ({ ...tag })),
        };
      }
      case "set_session_organization":
      case "update_timeline_session": {
        const sessionId = Number(args.sessionId);
        if (!timelineEntries.some((entry) => entry.sessionId === sessionId)) {
          throw new Error("session was not found");
        }
        if (timelineEntries.find((entry) => entry.sessionId === sessionId)?.isOpen) {
          throw new Error("projects and tags can only be assigned to closed sessions");
        }
        const organizationChanged = command === "set_session_organization"
          || Boolean(args.organizationChanged);
        let organization = sessionOrganizations.get(sessionId) ?? { project: null, tags: [] };
        if (organizationChanged) {
          const projectId = args.projectId === null ? null : Number(args.projectId);
          const tagIds = [...new Set(args.tagIds as number[])];
          const project = projectId === null ? null : projects.find((item) => item.id === projectId);
          if (projectId !== null && (!project || project.archived)) {
            throw new Error(project ? "archived projects cannot be assigned" : "project was not found");
          }
          const tags = tagIds.map((tagId) => activityTags.find((item) => item.id === tagId));
          const missingTag = tags.some((tag) => !tag);
          const archivedTag = tags.some((tag) => tag?.archived);
          if (missingTag || archivedTag) {
            throw new Error(archivedTag ? "archived activity tags cannot be assigned" : "activity tag was not found");
          }
          organization = {
            project: project ?? null,
            tags: sortedTags(tags as (typeof activityTags)[number][]),
          };
          sessionOrganizations.set(sessionId, organization);
        }
        if (command === "update_timeline_session") {
          const entry = timelineEntries.find((item) => item.sessionId === sessionId)!;
          entry.startedAtMs = Number(args.startedAtMs);
          entry.endedAtMs = Number(args.endedAtMs);
          entry.durationMs = entry.endedAtMs - entry.startedAtMs;
          return null;
        }
        return {
          project: organization.project ? { ...organization.project } : null,
          tags: organization.tags.map((tag) => ({ ...tag })),
        };
      }
      case "set_sessions_organization": {
        const sessionIds = [...new Set(args.sessionIds as number[])];
        if (sessionIds.length === 0) return { affectedCount: 0, undoToken: null };
        const projectId = args.projectId === null ? null : Number(args.projectId);
        const tagIds = [...new Set(args.tagIds as number[])];
        if (sessionIds.some((sessionId) => !timelineEntries.some((entry) => entry.sessionId === sessionId))) {
          throw new Error("session was not found");
        }
        if (sessionIds.some((sessionId) => timelineEntries.find((entry) => entry.sessionId === sessionId)?.isOpen)) {
          throw new Error("open sessions cannot be changed");
        }
        const project = projectId === null ? null : projects.find((item) => item.id === projectId);
        const tags = tagIds.map((tagId) => activityTags.find((item) => item.id === tagId));
        if (projectId !== null && (!project || project.archived)) {
          throw new Error(project ? "archived projects cannot be assigned" : "project was not found");
        }
        const missingTag = tags.some((tag) => !tag);
        const archivedTag = tags.some((tag) => tag?.archived);
        if (missingTag || archivedTag) {
          throw new Error(archivedTag ? "archived activity tags cannot be assigned" : "activity tag was not found");
        }
        const token = `organization-${Date.now()}-${nextOrganizationUndoId++}`;
        undoOrganizations.set(token, new Map(sessionIds.map((sessionId) => [
          sessionId,
          sessionOrganizations.get(sessionId) ?? { project: null, tags: [] },
        ])));
        for (const sessionId of sessionIds) {
          sessionOrganizations.set(sessionId, {
            project: project ?? null,
            tags: sortedTags(tags as (typeof activityTags)[number][]),
          });
        }
        undoHistory.push({
          token,
          createdAtMs: Date.now(),
          sessionCount: sessionIds.length,
          operationLabel: "Updated session organization",
        });
        return { affectedCount: sessionIds.length, undoToken: token };
      }
      case "search_timeline_range": {
        const offset = Number(args.offset ?? 0);
        const limit = Number(args.limit ?? 200);
        const filtered = filteredTimelineEntries(
          (args.filters ?? {}) as Record<string, unknown>,
        );
        const entries = filtered.slice(offset, offset + limit);
        return {
          entries,
          totalCount: filtered.length,
          activeDurationMs: filtered
            .filter((entry) => entry.state === "ACTIVE")
            .reduce((total, entry) => total + entry.durationMs, 0),
          idleDurationMs: filtered
            .filter((entry) => entry.state === "IDLE")
            .reduce((total, entry) => total + entry.durationMs, 0),
          offset,
          hasMore: offset + entries.length < filtered.length,
        };
      }
      case "get_timeline_undo_history": return undoHistory.map((entry) => ({ ...entry }));
      case "undo_timeline_edit": {
        const token = String(args.undoToken);
        const snapshot = undoOrganizations.get(token);
        if (!snapshot) throw new Error("undo is no longer available");
        for (const [sessionId, organization] of snapshot) {
          sessionOrganizations.set(sessionId, organization);
        }
        undoOrganizations.delete(token);
        const index = undoHistory.findIndex((entry) => entry.token === token);
        if (index >= 0) undoHistory.splice(index, 1);
        return snapshot.size;
      }
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
