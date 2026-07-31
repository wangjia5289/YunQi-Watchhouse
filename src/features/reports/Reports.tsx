import { useEffect, useMemo, useState } from "react";
import { dateFromLocalIso, formatDuration, localIsoDate, shiftLocalDate } from "../../lib/format";
import {
  ProductivityReport,
  FocusPlanHistorySummary,
  errorMessage,
  exportProductivityReportCsv,
  getFocusPlanHistory,
  getProductivityReport,
  startFocusPlan,
} from "../../lib/ipc";

type ReportPeriod = "week" | "month" | "custom";

function reportRange(
  period: ReportPeriod,
  customStart: string,
  customEnd: string,
): { startMs: number; endMs: number } {
  if (period === "custom") {
    const start = dateFromLocalIso(customStart);
    start.setHours(0, 0, 0, 0);
    const end = dateFromLocalIso(shiftLocalDate(customEnd, 1));
    end.setHours(0, 0, 0, 0);
    return {
      startMs: start.getTime(),
      endMs: end.getTime(),
    };
  }
  const now = new Date();
  const end = new Date(now);
  end.setHours(23, 59, 59, 999);
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  if (period === "week") {
    const day = (start.getDay() + 6) % 7;
    start.setDate(start.getDate() - day);
  } else {
    start.setDate(1);
  }
  return { startMs: start.getTime(), endMs: end.getTime() + 1 };
}

function comparison(current: number, previous: number): string {
  if (previous === 0) return current === 0 ? "No change" : "New activity";
  const percent = Math.round(((current - previous) / previous) * 100);
  return `${percent >= 0 ? "+" : ""}${percent}% vs previous`;
}

export function Reports() {
  const today = localIsoDate();
  const [period, setPeriod] = useState<ReportPeriod>("week");
  const [customStart, setCustomStart] = useState(shiftLocalDate(today, -6));
  const [customEnd, setCustomEnd] = useState(today);
  const [report, setReport] = useState<ProductivityReport | null>(null);
  const [focusHistory, setFocusHistory] = useState<FocusPlanHistorySummary | null>(null);
  const [focusMessage, setFocusMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const range = useMemo(
    () => reportRange(period, customStart, customEnd),
    [customEnd, customStart, period],
  );

  useEffect(() => {
    setError(null);
    void getProductivityReport(range.startMs, range.endMs)
      .then(setReport)
      .catch((reason) => setError(errorMessage(reason)));
    void getFocusPlanHistory(range.startMs, range.endMs)
      .then(setFocusHistory)
      .catch((reason) => setError(errorMessage(reason)));
  }, [range.endMs, range.startMs]);

  const dailyMaximum = Math.max(
    1,
    ...(report?.dailyUsage.map((day) => day.activeDurationMs) ?? []),
  );
  const hourlyMaximum = Math.max(
    1,
    ...(report?.hourlyUsage.map((hour) => hour.activeDurationMs) ?? []),
  );
  const categoryTotal = report?.categoryUsage.reduce(
    (sum, category) => sum + category.durationMs,
    0,
  ) ?? 0;
  const focusPlanCount = (focusHistory?.completedCount ?? 0)
    + (focusHistory?.cancelledCount ?? 0);
  const focusCompletionRate = focusPlanCount
    ? Math.round((focusHistory?.completedCount ?? 0) / focusPlanCount * 100)
    : 0;

  return (
    <div className="reports-page">
      <header className="reports-header">
        <div>
          <p className="date-label">Patterns and progress</p>
          <h1>Reports</h1>
        </div>
        <div className="range-tabs" role="group" aria-label="Report period">
          <button className={period === "week" ? "active" : ""} onClick={() => setPeriod("week")}>
            This week
          </button>
          <button className={period === "month" ? "active" : ""} onClick={() => setPeriod("month")}>
            This month
          </button>
          <button className={period === "custom" ? "active" : ""} onClick={() => setPeriod("custom")}>
            Custom
          </button>
        </div>
      </header>

      {period === "custom" && (
        <div className="report-custom-range">
          <label>
            <span>From</span>
            <input
              type="date"
              value={customStart}
              max={customEnd}
              onChange={(event) => {
                if (event.currentTarget.value) setCustomStart(event.currentTarget.value);
              }}
            />
          </label>
          <label>
            <span>To</span>
            <input
              type="date"
              value={customEnd}
              min={customStart}
              max={today}
              onChange={(event) => {
                if (event.currentTarget.value) setCustomEnd(event.currentTarget.value);
              }}
            />
          </label>
        </div>
      )}
      <div className="report-actions">
        <button type="button" onClick={() => {
          setFocusMessage(null);
          void exportProductivityReportCsv(range.startMs, range.endMs)
            .then((path) => {
              if (path) setFocusMessage(`Report saved to ${path}`);
            })
            .catch((reason) => setFocusMessage(errorMessage(reason)));
        }}>Export CSV</button>
      </div>

      {error && <div className="error-banner" role="alert">{error}</div>}

      <section className="report-metrics" aria-label="Report totals">
        <article>
          <span>Active time</span>
          <strong>{formatDuration(report?.activeDurationMs ?? 0)}</strong>
          <small>{comparison(report?.activeDurationMs ?? 0, report?.previousActiveDurationMs ?? 0)}</small>
        </article>
        <article>
          <span>Idle time</span>
          <strong>{formatDuration(report?.idleDurationMs ?? 0)}</strong>
          <small>{comparison(report?.idleDurationMs ?? 0, report?.previousIdleDurationMs ?? 0)}</small>
        </article>
        <article>
          <span>Daily average</span>
          <strong>{formatDuration((report?.activeDurationMs ?? 0) / Math.max(1, report?.dailyUsage.length ?? 1))}</strong>
          <small>{report?.dailyUsage.length ?? 0} recorded days</small>
        </article>
      </section>

      <section className="report-section focus-history" aria-labelledby="focus-history-title">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Focus plans</p>
            <h2 id="focus-history-title">Plan history</h2>
          </div>
        </div>
        <div className="focus-history-metrics">
          <div><span>Completion rate</span><strong>{focusCompletionRate}%</strong></div>
          <div><span>Completed</span><strong>{focusHistory?.completedCount ?? 0}</strong></div>
          <div><span>Cancelled</span><strong>{focusHistory?.cancelledCount ?? 0}</strong></div>
          <div><span>Focused time</span><strong>{formatDuration(focusHistory?.totalActualDurationMs ?? 0)}</strong></div>
          <div><span>Longest streak</span><strong>{focusHistory?.longestCompletedStreakDays ?? 0}d</strong></div>
        </div>
        {focusMessage && <p className="focus-history-message" role="status">{focusMessage}</p>}
        {focusHistory?.recentPlans.length ? (
          <div className="focus-history-list">
            {focusHistory.recentPlans.slice(0, 8).map((plan) => {
              const plannedMinutes = plan.plannedEndAtMs === null
                ? null
                : Math.round((plan.plannedEndAtMs - plan.startedAtMs) / 60_000);
              const actualDuration = Math.max(
                0,
                plan.endedAtMs - plan.startedAtMs - plan.pausedDurationMs,
              );
              return (
                <div key={plan.id}>
                  <span className={`focus-outcome ${plan.outcome.toLowerCase()}`}>
                    {plan.outcome === "COMPLETED" ? "Completed" : "Ended early"}
                  </span>
                  <span>
                    <strong>{new Date(plan.startedAtMs).toLocaleString(undefined, {
                      month: "short",
                      day: "numeric",
                      hour: "numeric",
                      minute: "2-digit",
                    })}</strong>
                    <small>{plannedMinutes ? `${plannedMinutes}m planned · ` : ""}{formatDuration(actualDuration)} focused</small>
                  </span>
                  {plannedMinutes !== null && plannedMinutes >= 5 && plannedMinutes <= 240 && (
                    <button type="button" onClick={() => {
                      setFocusMessage(null);
                      void startFocusPlan(plannedMinutes)
                        .then(() => setFocusMessage(`${plannedMinutes}-minute focus plan started.`))
                        .catch((reason) => setFocusMessage(errorMessage(reason)));
                    }}>Repeat</button>
                  )}
                </div>
              );
            })}
          </div>
        ) : (
          <p className="report-empty">No focus plans ended in this period.</p>
        )}
      </section>

      <section className="report-section" aria-labelledby="daily-report-title">
        <div className="section-heading">
          <div><p className="section-kicker">Trend</p><h2 id="daily-report-title">Daily active time</h2></div>
        </div>
        {report?.dailyUsage.length ? (
          <div className="report-daily-bars">
            {report.dailyUsage.map((day) => (
            <div key={day.date}>
              <span><i style={{ height: `${Math.max(4, day.activeDurationMs / dailyMaximum * 100)}%` }} /></span>
              <strong>{new Date(`${day.date}T12:00:00`).toLocaleDateString(undefined, { weekday: "short" })}</strong>
              <small>{formatDuration(day.activeDurationMs)}</small>
            </div>
            ))}
          </div>
        ) : (
          <p className="report-empty">No activity recorded in this period.</p>
        )}
      </section>

      <section className="report-section" aria-labelledby="hourly-report-title">
        <div className="section-heading">
          <div><p className="section-kicker">Rhythm</p><h2 id="hourly-report-title">Active time by hour</h2></div>
        </div>
        <div className="report-heatmap">
          {report?.hourlyUsage.map((hour) => (
            <span
              key={hour.hour}
              style={{ opacity: 0.18 + hour.activeDurationMs / hourlyMaximum * 0.82 }}
              title={`${String(hour.hour).padStart(2, "0")}:00 · ${formatDuration(hour.activeDurationMs)}`}
            >
              {hour.hour}
            </span>
          ))}
        </div>
      </section>

      <section className="report-section" aria-labelledby="category-report-title">
        <div className="section-heading">
          <div><p className="section-kicker">Allocation</p><h2 id="category-report-title">Categories</h2></div>
        </div>
        <div className="report-categories">
          {report?.categoryUsage.map((category) => (
            <div key={category.category}>
              <span>{category.category}</span>
              <i><b style={{ width: `${categoryTotal ? category.durationMs / categoryTotal * 100 : 0}%` }} /></i>
              <strong>{formatDuration(category.durationMs)}</strong>
            </div>
          ))}
          {!report?.categoryUsage.length && <p>No categorized activity in this period.</p>}
        </div>
      </section>
    </div>
  );
}
