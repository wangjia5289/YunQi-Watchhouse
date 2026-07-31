import { describe, expect, it } from "vitest";
import {
  applyUsageLimitDailyException,
  formatReminderLocalDate,
  hasActiveUsageLimitSnooze,
} from "./UsageLimitReminderCenter";
import { UsageLimitProgress } from "../../lib/ipc";

const progress: UsageLimitProgress = {
  id: 18,
  scopeType: "APPLICATION",
  applicationId: 42,
  applicationName: "Safari",
  category: null,
  weekdayLimitMinutes: 90,
  weekendLimitMinutes: 120,
  notificationsEnabled: true,
  enabled: true,
  localDate: "2026-07-31",
  limitMinutes: 90,
  baseLimitMinutes: 90,
  temporaryAddedMinutes: 0,
  notificationsSnoozedUntilMs: null,
  notificationsSilenced: false,
  usedDurationMs: 30 * 60_000,
  percentage: 33.3,
  thresholdState: "BELOW_80",
};

describe("applyUsageLimitDailyException", () => {
  it("updates only the matching rule and derives its effective limit from the base limit", () => {
    const exception = {
      ruleId: 18,
      localDate: "2026-07-31",
      temporaryAddedMinutes: 30,
      notificationsSnoozedUntilMs: 1_785_497_400_000,
      notificationsSilenced: false,
    };

    expect(applyUsageLimitDailyException(progress, exception)).toMatchObject({
      limitMinutes: 120,
      baseLimitMinutes: 90,
      temporaryAddedMinutes: 30,
      notificationsSnoozedUntilMs: 1_785_497_400_000,
    });
    expect(applyUsageLimitDailyException({ ...progress, id: 19 }, exception)).toEqual({
      ...progress,
      id: 19,
    });
  });
});

describe("formatReminderLocalDate", () => {
  it("formats a local ISO date without parsing it as UTC", () => {
    expect(formatReminderLocalDate("2026-07-31", "en")).toContain("2026");
    expect(formatReminderLocalDate("2026-07-31", "zh-CN")).toContain("2026");
  });
});

describe("hasActiveUsageLimitSnooze", () => {
  it("does not present expired snoozes as active", () => {
    expect(hasActiveUsageLimitSnooze(500, 500)).toBe(false);
    expect(hasActiveUsageLimitSnooze(501, 500)).toBe(true);
    expect(hasActiveUsageLimitSnooze(null, 500)).toBe(false);
  });
});
