import { describe, expect, it } from "vitest";
import { CurrentActivity, FocusModeStatus, UsageLimitProgress } from "../../lib/ipc";
import {
  closestUsageLimit,
  currentApplicationName,
  focusElapsedMs,
  focusRemainingMs,
  usageLimitName,
} from "./trayModel";

function focus(overrides: Partial<FocusModeStatus> = {}): FocusModeStatus {
  return {
    active: true,
    startedAtMs: 1_000,
    plannedEndAtMs: 61_000,
    paused: false,
    pausedAtMs: null,
    totalPausedMs: 5_000,
    templateId: null,
    ...overrides,
  };
}

function limit(id: number, percentage: number, enabled = true): UsageLimitProgress {
  return {
    id,
    scopeType: "APPLICATION",
    applicationId: id,
    applicationName: `App ${id}`,
    category: null,
    weekdayLimitMinutes: 60,
    weekendLimitMinutes: 60,
    notificationsEnabled: true,
    enabled,
    localDate: "2026-07-31",
    limitMinutes: 60,
    baseLimitMinutes: 60,
    temporaryAddedMinutes: 0,
    notificationsSnoozedUntilMs: null,
    notificationsSilenced: false,
    usedDurationMs: 30_000,
    percentage,
    thresholdState: "BELOW_80",
  };
}

describe("tray panel model", () => {
  it("only reports an application while tracking is running", () => {
    const current: CurrentActivity = {
      paused: false,
      persistence: { status: "RUNNING" },
      monitor: {
        status: "RUNNING",
        payload: {
          observedAtMs: 10,
          state: "ACTIVE",
          idleDurationMs: 0,
          lastInputAtMs: 10,
          foregroundApplication: {
            name: "Code",
            bundleIdentifier: "com.microsoft.VSCode",
            executablePath: null,
          },
        },
      },
    };
    expect(currentApplicationName(current)).toBe("Code");
    expect(currentApplicationName({ ...current, paused: true })).toBeNull();
  });

  it("selects the highest enabled usage limit", () => {
    const result = closestUsageLimit([limit(1, 95, false), limit(2, 64), limit(3, 81)]);
    expect(result?.id).toBe(3);
    expect(usageLimitName(result!)).toBe("App 3");
    expect(closestUsageLimit([limit(1, 95, false)])).toBeNull();
  });

  it("subtracts completed and current pauses from focus timing", () => {
    expect(focusElapsedMs(focus(), 31_000)).toBe(25_000);
    const paused = focus({ paused: true, pausedAtMs: 21_000 });
    expect(focusElapsedMs(paused, 31_000)).toBe(15_000);
    expect(focusRemainingMs(paused, 31_000)).toBe(40_000);
    expect(focusRemainingMs(focus({ plannedEndAtMs: null }), 31_000)).toBeNull();
  });
});
