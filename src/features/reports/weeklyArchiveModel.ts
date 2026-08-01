import {
  FocusPlanHistorySummary,
  ProductivityReport,
  WeeklyReportArchiveInput,
} from "../../lib/ipc";
import { localIsoDate, shiftLocalDate } from "../../lib/format";
import { buildWeeklyInsights } from "./weeklyInsightModel";

export function buildWeeklyArchiveInput(
  report: ProductivityReport,
  focusHistory: FocusPlanHistorySummary | null,
  previousWeekActiveDurationMs: number,
  generatedAtMs = Date.now(),
): WeeklyReportArchiveInput {
  const insights = buildWeeklyInsights(report, focusHistory, previousWeekActiveDurationMs);
  const weekStartDate = localIsoDate(new Date(report.range.startMs));
  return {
    weekStartDate,
    weekEndDate: shiftLocalDate(weekStartDate, 6),
    generatedAtMs,
    activeDurationMs: report.activeDurationMs,
    idleDurationMs: report.idleDurationMs,
    previousWeekActiveDurationMs,
    strongestDayDate: insights.bestDay?.date ?? null,
    peakHour: insights.peakHour,
    leadingCategory: insights.topCategory?.category ?? null,
    focusCompletionRate: insights.focusCompletionRate,
    payloadJson: JSON.stringify({ report, focusHistory, insights }),
  };
}
