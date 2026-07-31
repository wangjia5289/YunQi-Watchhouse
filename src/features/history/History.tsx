import { useEffect, useMemo, useState } from "react";
import {
  dateFromLocalIso,
  formatDuration,
  localIsoDate,
  shiftLocalDate,
} from "../../lib/format";
import {
  DailyUsage,
  AppUsage,
  errorMessage,
  getAppUsage,
  getDailyUsage,
} from "../../lib/ipc";
import { ACTIVITY_DATA_CHANGED } from "../../lib/events";
import { useLocale } from "../../lib/i18n";

type Period = "day" | "week" | "month";

function dayStart(date: string) {
  const value = dateFromLocalIso(date);
  value.setHours(0, 0, 0, 0);
  return value.getTime();
}

function rangeFor(period: Period, anchor: string) {
  const date = dateFromLocalIso(anchor);
  let start = anchor;
  let days = 1;
  if (period === "week") {
    const mondayOffset = (date.getDay() + 6) % 7;
    start = shiftLocalDate(anchor, -mondayOffset);
    days = 7;
  } else if (period === "month") {
    start = `${anchor.slice(0, 7)}-01`;
    const next = dateFromLocalIso(start);
    next.setMonth(next.getMonth() + 1);
    return { start, end: localIsoDate(next) };
  }
  return { start, end: shiftLocalDate(start, days) };
}

function shiftPeriod(period: Period, anchor: string, direction: -1 | 1): string {
  if (period !== "month") {
    return shiftLocalDate(anchor, direction * (period === "week" ? 7 : 1));
  }
  const value = dateFromLocalIso(`${anchor.slice(0, 7)}-01`);
  value.setMonth(value.getMonth() + direction);
  return localIsoDate(value);
}

export function History() {
  const { locale, t } = useLocale();
  const dateLocale = locale === "zh-CN" ? "zh-CN" : "en";
  const today = localIsoDate();
  const [period, setPeriod] = useState<Period>("week");
  const [anchor, setAnchor] = useState(today);
  const [days, setDays] = useState<DailyUsage[]>([]);
  const [apps, setApps] = useState<AppUsage[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [dataRevision, setDataRevision] = useState(0);
  const range = useMemo(() => rangeFor(period, anchor), [anchor, period]);

  useEffect(() => {
    const refresh = () => setDataRevision((revision) => revision + 1);
    window.addEventListener(ACTIVITY_DATA_CHANGED, refresh);
    return () => window.removeEventListener(ACTIVITY_DATA_CHANGED, refresh);
  }, []);

  useEffect(() => {
    let active = true;
    const startMs = dayStart(range.start);
    const endMs = dayStart(range.end);
    void Promise.all([getDailyUsage(startMs, endMs), getAppUsage(startMs, endMs)])
      .then(([daily, usage]) => {
        if (!active) return;
        const byDate = new Map(daily.map((day) => [day.date, day]));
        const complete: DailyUsage[] = [];
        for (let date = range.start; date < range.end; date = shiftLocalDate(date, 1)) {
          complete.push(byDate.get(date) ?? {
            date,
            activeDurationMs: 0,
            idleDurationMs: 0,
          });
        }
        setDays(complete);
        setApps(usage);
        setError(null);
      })
      .catch((reason) => active && setError(errorMessage(reason)));
    return () => {
      active = false;
    };
  }, [dataRevision, range]);

  const total = days.reduce((sum, day) => sum + day.activeDurationMs, 0);
  const average = days.length ? total / days.length : 0;
  const maximum = Math.max(
    1,
    ...days.map((day) => day.activeDurationMs + day.idleDurationMs),
  );
  const periodTitle =
    period === "day"
      ? new Intl.DateTimeFormat(dateLocale, { dateStyle: "long" }).format(
          dateFromLocalIso(range.start),
        )
      : period === "month"
        ? new Intl.DateTimeFormat(dateLocale, { month: "long", year: "numeric" }).format(
            dateFromLocalIso(range.start),
          )
        : `${new Intl.DateTimeFormat(dateLocale, { month: "short", day: "numeric" }).format(
            dateFromLocalIso(range.start),
          )} – ${new Intl.DateTimeFormat(dateLocale, {
            month: "short",
            day: "numeric",
            year: "numeric",
          }).format(dateFromLocalIso(shiftLocalDate(range.end, -1)))}`;
  const isCurrentPeriod = today >= range.start && today < range.end;

  return (
    <div className="history-page">
      <header className="history-header">
        <div><p className="date-label">{t("Long-term patterns")}</p><h1>{t("History")}</h1></div>
        <div className="range-tabs">
          {(["day", "week", "month"] as const).map((value) => (
            <button key={value} className={period === value ? "active" : ""}
              onClick={() => setPeriod(value)}>{t(value[0].toUpperCase() + value.slice(1))}</button>
          ))}
        </div>
      </header>
      <div className="history-controls">
        <button onClick={() => setAnchor(shiftPeriod(period, anchor, -1))}>{t("← Previous")}</button>
        <input type="date" value={anchor} max={today}
          onChange={(event) => event.currentTarget.value && setAnchor(event.currentTarget.value)} />
        <strong className="history-period-title">{periodTitle}</strong>
        <button disabled={isCurrentPeriod}
          onClick={() => setAnchor(shiftPeriod(period, anchor, 1))}>{t("Next →")}</button>
        <button disabled={isCurrentPeriod} onClick={() => setAnchor(today)}>{t("Today")}</button>
      </div>
      {error && <div className="error-banner">{t(error)}</div>}
      <section className="applications-overview">
        <article><p>{t("Average daily active time")}</p><strong>{formatDuration(average, locale)}</strong></article>
        <article><p>{t("Total active time")}</p><strong>{formatDuration(total, locale)}</strong></article>
        <article><p>{t("Most used app")}</p><strong>{apps[0]?.applicationName ?? "—"}</strong></article>
      </section>
      <section className="history-chart">
        <div className="list-heading"><div><p className="section-kicker">{t("Active and idle")}</p><h2>{t("Daily activity")}</h2></div>
          <span className="history-legend"><i className="active" />{t("Active")} <i className="idle" />{t("Idle")}</span>
        </div>
        <div className="history-bars">
          {days.length ? days.map((day) => (
            <div className="history-bar" key={day.date}>
              <strong>{formatDuration(day.activeDurationMs, locale)}</strong>
              <span className="history-bar-stack">
                <i className="idle" style={{ height: `${day.idleDurationMs / maximum * 100}%` }} />
                <i className="active" style={{
                  height: day.activeDurationMs > 0
                    ? `${Math.max(4, day.activeDurationMs / maximum * 100)}%`
                    : "0",
                }} />
              </span>
              <small>{new Intl.DateTimeFormat(dateLocale, { weekday: "short", day: "numeric" }).format(dateFromLocalIso(day.date))}</small>
            </div>
          )) : <div className="empty-applications"><h2>{t("No history yet")}</h2><p>{t("Recorded activity will appear here.")}</p></div>}
        </div>
      </section>
    </div>
  );
}
