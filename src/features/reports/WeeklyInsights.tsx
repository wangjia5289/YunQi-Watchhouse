import { formatDuration } from "../../lib/format";
import { FocusPlanHistorySummary, ProductivityReport } from "../../lib/ipc";
import { Locale, useLocale } from "../../lib/i18n";
import { buildWeeklyInsights, WeeklyInsightSummary } from "./weeklyInsightModel";
import "./WeeklyInsights.css";

function trendCopy(insights: WeeklyInsightSummary, locale: Locale): string {
  if (locale === "zh-CN") {
    if (insights.activityTrend === "none") return "本周还没有活跃记录。";
    if (insights.activityTrend === "new") return "这是第一周活跃记录。";
    if (insights.activityTrend === "steady") return "与上周基本持平。";
    return `活跃时长较上周${insights.activityTrend === "up" ? "增加" : "减少"} ${Math.abs(insights.activityChangePercent ?? 0)}%。`;
  }
  if (insights.activityTrend === "none") return "No active time has been recorded this week.";
  if (insights.activityTrend === "new") return "This is the first week with recorded activity.";
  if (insights.activityTrend === "steady") return "Active time is steady compared with last week.";
  return `Active time ${insights.activityTrend === "up" ? "increased" : "decreased"} by ${Math.abs(insights.activityChangePercent ?? 0)}% from last week.`;
}

export function WeeklyInsights({
  report,
  focusHistory,
  previousWeekActiveDurationMs,
}: {
  report: ProductivityReport;
  focusHistory: FocusPlanHistorySummary | null;
  previousWeekActiveDurationMs: number;
}) {
  const { locale, t } = useLocale();
  const dateLocale = locale === "zh-CN" ? "zh-CN" : "en";
  const insights = buildWeeklyInsights(report, focusHistory, previousWeekActiveDurationMs);
  const bestDay = insights.bestDay
    ? new Date(`${insights.bestDay.date}T12:00:00`).toLocaleDateString(dateLocale, {
      weekday: "long",
      month: "short",
      day: "numeric",
    })
    : t("No data");
  const peakRange = insights.peakHour === null
    ? t("No data")
    : `${String(insights.peakHour).padStart(2, "0")}:00 - ${String((insights.peakHour + 1) % 24).padStart(2, "0")}:00`;

  return (
    <section className="report-section weekly-insights" aria-labelledby="weekly-insights-title">
      <div className="section-heading">
        <div>
          <p className="section-kicker">{t("Local intelligence")}</p>
          <h2 id="weekly-insights-title">{t("Weekly insights")}</h2>
        </div>
        <span>{t("Calculated privately on this Mac")}</span>
      </div>
      <div className="weekly-insight-grid">
        <article>
          <span>{t("Week over week")}</span>
          <strong>{insights.activityChangePercent === null
            ? insights.activityTrend === "new" ? t("New") : "-"
            : `${insights.activityChangePercent > 0 ? "+" : ""}${insights.activityChangePercent}%`}</strong>
          <small>{trendCopy(insights, locale)}</small>
        </article>
        <article>
          <span>{t("Most active day")}</span>
          <strong>{bestDay}</strong>
          <small>{insights.bestDay
            ? formatDuration(insights.bestDay.activeDurationMs, locale)
            : t("More activity will reveal this pattern.")}</small>
        </article>
        <article>
          <span>{t("Peak hour")}</span>
          <strong>{peakRange}</strong>
          <small>{insights.peakHour === null
            ? t("More activity will reveal this pattern.")
            : `${formatDuration(insights.peakHourDurationMs, locale)} ${t("active")}`}</small>
        </article>
        <article>
          <span>{t("Leading category")}</span>
          <strong>{insights.topCategory?.category ?? t("No data")}</strong>
          <small>{insights.topCategory
            ? locale === "zh-CN"
              ? `占活跃时长的 ${insights.topCategoryShare}%`
              : `${insights.topCategoryShare}% of active time`
            : t("Add categories or classification rules to see this pattern.")}</small>
        </article>
        <article>
          <span>{t("Focus plan follow-through")}</span>
          <strong>{insights.focusCompletionRate === null ? "-" : `${insights.focusCompletionRate}%`}</strong>
          <small>{insights.focusCompletionRate === null
            ? t("Complete a focus plan to see this pattern.")
            : t("Completed plans in this week")}</small>
        </article>
      </div>
    </section>
  );
}
