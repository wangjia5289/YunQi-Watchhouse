import { describe, expect, it } from "vitest";
import { formatLimitMinutes, usageLimitInputFromDraft } from "./UsageLimits";

describe("usageLimitInputFromDraft", () => {
  it("creates an application limit without carrying category data", () => {
    expect(usageLimitInputFromDraft({
      scopeType: "APPLICATION",
      applicationId: "42",
      category: "Work",
      weekdayLimitMinutes: "120",
      weekendLimitMinutes: "90",
      notificationsEnabled: true,
      enabled: true,
    })).toEqual({
      scopeType: "APPLICATION",
      applicationId: 42,
      category: null,
      weekdayLimitMinutes: 120,
      weekendLimitMinutes: 90,
      notificationsEnabled: true,
      enabled: true,
    });
  });

  it("trims a category target without carrying an application id", () => {
    expect(usageLimitInputFromDraft({
      scopeType: "CATEGORY",
      applicationId: "42",
      category: "  Deep Work  ",
      weekdayLimitMinutes: "60",
      weekendLimitMinutes: "30",
      notificationsEnabled: false,
      enabled: true,
    })).toEqual({
      scopeType: "CATEGORY",
      applicationId: null,
      category: "Deep Work",
      weekdayLimitMinutes: 60,
      weekendLimitMinutes: 30,
      notificationsEnabled: false,
      enabled: true,
    });
  });

  it("rejects invalid targets and minute ranges", () => {
    expect(usageLimitInputFromDraft({
      scopeType: "APPLICATION",
      applicationId: "",
      category: "",
      weekdayLimitMinutes: "60",
      weekendLimitMinutes: "60",
      notificationsEnabled: true,
      enabled: true,
    })).toBeNull();
    expect(usageLimitInputFromDraft({
      scopeType: "CATEGORY",
      applicationId: "",
      category: "Work",
      weekdayLimitMinutes: "0",
      weekendLimitMinutes: "1441",
      notificationsEnabled: true,
      enabled: true,
    })).toBeNull();
  });

  it("counts category length by characters instead of UTF-16 code units", () => {
    expect(usageLimitInputFromDraft({
      scopeType: "CATEGORY",
      applicationId: "",
      category: "专注".repeat(20),
      weekdayLimitMinutes: "60",
      weekendLimitMinutes: "60",
      notificationsEnabled: true,
      enabled: true,
    })).not.toBeNull();
    expect(usageLimitInputFromDraft({
      scopeType: "CATEGORY",
      applicationId: "",
      category: "专注".repeat(20) + "额",
      weekdayLimitMinutes: "60",
      weekendLimitMinutes: "60",
      notificationsEnabled: true,
      enabled: true,
    })).toBeNull();
  });
});

describe("formatLimitMinutes", () => {
  it("uses compact English and complete Chinese duration labels", () => {
    expect(formatLimitMinutes(135, "en")).toBe("2h 15m");
    expect(formatLimitMinutes(135, "zh-CN")).toBe("2 小时 15 分钟");
    expect(formatLimitMinutes(45, "zh-CN")).toBe("45 分钟");
  });
});
