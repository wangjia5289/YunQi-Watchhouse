import { describe, expect, it } from "vitest";
import { FocusPlanHistorySummary, ProductivityReport } from "../../lib/ipc";
import { buildWeeklyInsights, shiftReportRangeByDays } from "./weeklyInsightModel";

const report: ProductivityReport = {
  range: { startMs: 0, endMs: 1 },
  activeDurationMs: 12_000,
  idleDurationMs: 4_000,
  previousActiveDurationMs: 10_000,
  previousIdleDurationMs: 5_000,
  dailyUsage: [
    { date: "2026-07-27", activeDurationMs: 4_000, idleDurationMs: 0 },
    { date: "2026-07-28", activeDurationMs: 8_000, idleDurationMs: 0 },
  ],
  hourlyUsage: [
    { hour: 9, activeDurationMs: 3_000 },
    { hour: 14, activeDurationMs: 7_000 },
  ],
  categoryUsage: [
    { category: "Work", durationMs: 9_000, applicationCount: 2 },
    { category: "Learning", durationMs: 3_000, applicationCount: 1 },
  ],
  organizationInsights: {
    projectUsage: [],
    tagUsage: [],
    unassignedActiveDurationMs: 0,
    unassignedSessionCount: 0,
  },
};

const focusHistory: FocusPlanHistorySummary = {
  completedCount: 3,
  cancelledCount: 1,
  totalPlannedDurationMs: 0,
  totalActualDurationMs: 0,
  totalPausedDurationMs: 0,
  longestCompletedStreakDays: 2,
  recentPlans: [],
};

describe("buildWeeklyInsights", () => {
  it("finds weekly peaks and progress", () => {
    expect(buildWeeklyInsights(report, focusHistory)).toEqual({
      activityTrend: "up",
      activityChangePercent: 20,
      bestDay: report.dailyUsage[1],
      topCategory: report.categoryUsage[0],
      topCategoryShare: 75,
      peakHour: 14,
      peakHourDurationMs: 7_000,
      focusCompletionRate: 75,
    });
  });

  it("compares the same weekdays from the previous week", () => {
    const insights = buildWeeklyInsights(report, focusHistory, 6_000);
    expect(insights.activityChangePercent).toBe(100);
    expect(insights.activityTrend).toBe("up");
  });

  it("shifts both range boundaries by calendar days", () => {
    const current = {
      startMs: new Date(2026, 6, 27, 0, 0, 0, 0).getTime(),
      endMs: new Date(2026, 7, 1, 0, 0, 0, 0).getTime(),
    };
    expect(shiftReportRangeByDays(current, -7)).toEqual({
      startMs: new Date(2026, 6, 20, 0, 0, 0, 0).getTime(),
      endMs: new Date(2026, 6, 25, 0, 0, 0, 0).getTime(),
    });
  });

  it("handles an empty first week", () => {
    const empty = buildWeeklyInsights({
      ...report,
      activeDurationMs: 0,
      previousActiveDurationMs: 0,
      dailyUsage: [],
      hourlyUsage: [],
      categoryUsage: [],
    }, null);
    expect(empty.activityTrend).toBe("none");
    expect(empty.bestDay).toBeNull();
    expect(empty.peakHour).toBeNull();
    expect(empty.focusCompletionRate).toBeNull();
  });
});
