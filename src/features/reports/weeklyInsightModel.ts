import { FocusPlanHistorySummary, ProductivityReport } from "../../lib/ipc";

export interface WeeklyInsightSummary {
  activityTrend: "none" | "new" | "steady" | "up" | "down";
  activityChangePercent: number | null;
  bestDay: ProductivityReport["dailyUsage"][number] | null;
  topCategory: ProductivityReport["categoryUsage"][number] | null;
  topCategoryShare: number;
  peakHour: number | null;
  peakHourDurationMs: number;
  focusCompletionRate: number | null;
}

export function shiftReportRangeByDays(
  range: ProductivityReport["range"],
  days: number,
): ProductivityReport["range"] {
  const start = new Date(range.startMs);
  const end = new Date(range.endMs);
  start.setDate(start.getDate() + days);
  end.setDate(end.getDate() + days);
  return { startMs: start.getTime(), endMs: end.getTime() };
}

export function buildWeeklyInsights(
  report: ProductivityReport,
  focusHistory: FocusPlanHistorySummary | null,
  previousWeekActiveDurationMs = report.previousActiveDurationMs,
): WeeklyInsightSummary {
  const previous = previousWeekActiveDurationMs;
  const current = report.activeDurationMs;
  const activityChangePercent = previous > 0
    ? Math.round(((current - previous) / previous) * 100)
    : null;
  const activityTrend = current === 0 && previous === 0
    ? "none"
    : previous === 0
      ? "new"
      : activityChangePercent === 0
        ? "steady"
        : (activityChangePercent ?? 0) > 0
          ? "up"
          : "down";

  const bestDay = report.dailyUsage.reduce<WeeklyInsightSummary["bestDay"]>(
    (best, day) => day.activeDurationMs > (best?.activeDurationMs ?? 0) ? day : best,
    null,
  );
  const topCategory = report.categoryUsage.reduce<WeeklyInsightSummary["topCategory"]>(
    (top, category) => category.durationMs > (top?.durationMs ?? 0) ? category : top,
    null,
  );
  const categoryTotal = report.categoryUsage.reduce(
    (total, category) => total + category.durationMs,
    0,
  );
  const peak = report.hourlyUsage.reduce<ProductivityReport["hourlyUsage"][number] | null>(
    (top, hour) => hour.activeDurationMs > (top?.activeDurationMs ?? 0) ? hour : top,
    null,
  );
  const planCount = (focusHistory?.completedCount ?? 0) + (focusHistory?.cancelledCount ?? 0);

  return {
    activityTrend,
    activityChangePercent,
    bestDay,
    topCategory,
    topCategoryShare: topCategory && categoryTotal > 0
      ? Math.round(topCategory.durationMs / categoryTotal * 100)
      : 0,
    peakHour: peak?.hour ?? null,
    peakHourDurationMs: peak?.activeDurationMs ?? 0,
    focusCompletionRate: planCount > 0
      ? Math.round((focusHistory?.completedCount ?? 0) / planCount * 100)
      : null,
  };
}
