import { CSSProperties, useEffect, useMemo, useState } from "react";
import {
  dateFromLocalIso,
  formatDuration,
  localIsoDate,
  shiftLocalDate,
} from "../../lib/format";
import {
  AppUsage,
  DailyUsage,
  errorMessage,
  getApplicationDailyUsage,
  updateApplicationPreferences,
} from "../../lib/ipc";
import { ACTIVITY_DATA_CHANGED, notifyActivityDataChanged } from "../../lib/events";
import { ApplicationRange, useApplications } from "./useApplications";
import { ApplicationIcon } from "./ApplicationIcon";
import { useLocale } from "../../lib/i18n";

type RangePreset = "today" | "7days" | "30days" | "custom";

function startOfLocalDay(date: string): number {
  const value = dateFromLocalIso(date);
  value.setHours(0, 0, 0, 0);
  return value.getTime();
}

function rangeForPreset(
  preset: RangePreset,
  customStart: string,
  customEnd: string,
): ApplicationRange {
  const today = localIsoDate();
  const startDate =
    preset === "today"
      ? today
      : preset === "7days"
        ? shiftLocalDate(today, -6)
        : preset === "30days"
          ? shiftLocalDate(today, -29)
          : customStart;
  const endDate = preset === "custom" ? customEnd : today;
  return {
    startMs: startOfLocalDay(startDate),
    endMs: startOfLocalDay(shiftLocalDate(endDate, 1)),
  };
}

function appHue(applicationId: number): number {
  return 135 + ((applicationId * 37) % 90);
}

function ApplicationRow({
  application,
  total,
  selected,
  onSelect,
}: {
  application: AppUsage;
  total: number;
  selected: boolean;
  onSelect: () => void;
}) {
  const { locale, t } = useLocale();
  const percentage = total > 0 ? (application.durationMs / total) * 100 : 0;
  const style = {
    "--usage-width": `${percentage}%`,
    "--app-hue": appHue(application.applicationId),
  } as CSSProperties;

  return (
    <button
      type="button"
      className={`application-row${selected ? " selected" : ""}`}
      onClick={onSelect}
      style={style}
    >
      <ApplicationIcon
        applicationId={application.applicationId}
        applicationName={application.applicationName}
      />
      <span className="application-main">
        <span className="application-copy">
          <strong>{application.applicationName}</strong>
          <small>{application.bundleIdentifier ?? t("Application")}</small>
        </span>
        <span className="usage-track">
          <i />
        </span>
      </span>
      <span className="application-duration">
        <strong>{formatDuration(application.durationMs, locale)}</strong>
        <small>{percentage.toFixed(1)}%</small>
      </span>
      <svg className="row-chevron" viewBox="0 0 24 24" aria-hidden="true">
        <path d="m9 5 7 7-7 7" />
      </svg>
    </button>
  );
}

export function Applications() {
  const { locale, t } = useLocale();
  const today = localIsoDate();
  const [preset, setPreset] = useState<RangePreset>("today");
  const [customStart, setCustomStart] = useState(shiftLocalDate(today, -6));
  const [customEnd, setCustomEnd] = useState(today);
  const range = useMemo(
    () => rangeForPreset(preset, customStart, customEnd),
    [customEnd, customStart, preset],
  );
  const { applications, categories, loading, error, refresh } = useApplications(range);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [categoryFilter, setCategoryFilter] = useState<string | null>(null);
  const visibleApplications = categoryFilter
    ? applications.filter((application) => application.category === categoryFilter)
    : applications;
  const total = visibleApplications.reduce(
    (sum, application) => sum + application.durationMs,
    0,
  );
  const selected =
    visibleApplications.find((application) => application.applicationId === selectedId) ??
    visibleApplications[0] ??
    null;
  const [trend, setTrend] = useState<DailyUsage[]>([]);
  const [dataRevision, setDataRevision] = useState(0);
  const [preferenceError, setPreferenceError] = useState<string | null>(null);
  const [savingPreferences, setSavingPreferences] = useState(false);
  const [categoryDraft, setCategoryDraft] = useState("");
  const rangeDays = Math.max(
    1,
    Math.round((range.endMs - range.startMs) / (24 * 60 * 60 * 1_000)),
  );

  useEffect(() => {
    const refreshTrend = () => setDataRevision((revision) => revision + 1);
    window.addEventListener(ACTIVITY_DATA_CHANGED, refreshTrend);
    return () => window.removeEventListener(ACTIVITY_DATA_CHANGED, refreshTrend);
  }, []);

  useEffect(() => {
    if (!selected) {
      setTrend([]);
      return;
    }
    let active = true;
    void getApplicationDailyUsage(
      selected.applicationId,
      range.startMs,
      range.endMs,
    ).then((usage) => {
      if (active) setTrend(usage);
    }).catch(() => {
      if (active) setTrend([]);
    });
    return () => {
      active = false;
    };
  }, [dataRevision, range.endMs, range.startMs, selected?.applicationId]);

  const visibleTrend = trend.slice(-14);
  const trendMaximum = Math.max(1, ...visibleTrend.map((day) => day.activeDurationMs));

  useEffect(() => {
    setCategoryDraft(selected?.category ?? "");
  }, [selected?.applicationId, selected?.category]);

  async function savePreferences(
    category: string,
    isIgnored: boolean,
    recordWindowTitles = selected?.recordWindowTitles ?? false,
  ) {
    if (!selected || savingPreferences) return;
    setSavingPreferences(true);
    setPreferenceError(null);
    try {
      await updateApplicationPreferences(
        selected.applicationId,
        category,
        isIgnored,
        recordWindowTitles,
      );
      if (categoryFilter === selected.category && category !== selected.category) {
        setCategoryFilter(category);
      }
      notifyActivityDataChanged();
      refresh();
    } catch (reason) {
      setPreferenceError(errorMessage(reason));
    } finally {
      setSavingPreferences(false);
    }
  }

  return (
    <div className="applications-page">
      <header className="applications-header">
        <div>
          <p className="date-label">{t("Time by application")}</p>
          <h1>{t("Applications")}</h1>
        </div>
        <div className="range-tabs" role="group" aria-label={t("Application usage range")}>
          {(
            [
              ["today", "Today"],
              ["7days", "7 Days"],
              ["30days", "30 Days"],
              ["custom", "Custom"],
            ] as const
          ).map(([value, label]) => (
            <button
              type="button"
              className={preset === value ? "active" : ""}
              onClick={() => setPreset(value)}
              key={value}
            >
              {t(label)}
            </button>
          ))}
        </div>
      </header>

      {preset === "custom" && (
        <div className="custom-range">
          <label>
            {t("From")}
            <input
              type="date"
              value={customStart}
              max={customEnd}
              onChange={(event) => setCustomStart(event.currentTarget.value)}
            />
          </label>
          <span>{t("to")}</span>
          <label>
            {t("Until")}
            <input
              type="date"
              value={customEnd}
              min={customStart}
              max={today}
              onChange={(event) => setCustomEnd(event.currentTarget.value)}
            />
          </label>
        </div>
      )}

      {error && (
        <div className="error-banner" role="alert">
          <span>{t(error)}</span>
          <button type="button" onClick={refresh}>
            {t("Try again")}
          </button>
        </div>
      )}

      <section className="applications-overview" aria-label={t("Application usage summary")}>
        <article>
          <p>{t("Total active time")}</p>
          <strong>{formatDuration(total, locale)}</strong>
        </article>
        <article>
          <p>{t("Applications used")}</p>
          <strong>{visibleApplications.length}</strong>
        </article>
        <article>
          <p>{t("Most used")}</p>
          <strong>{visibleApplications[0]?.applicationName ?? "—"}</strong>
        </article>
      </section>

      {categories.length > 0 && (
        <section className="category-usage" aria-label={t("Usage by category")}>
          <button
            type="button"
            className={categoryFilter === null ? "active" : ""}
            onClick={() => setCategoryFilter(null)}
          >
            <span>{t("All categories")}</span>
            <strong>{formatDuration(categories.reduce((sum, item) => sum + item.durationMs, 0), locale)}</strong>
          </button>
          {categories.map((category) => (
            <button
              type="button"
              className={categoryFilter === category.category ? "active" : ""}
              onClick={() => setCategoryFilter(category.category)}
              key={category.category}
            >
              <span>{category.category}</span>
              <strong>{formatDuration(category.durationMs, locale)}</strong>
              <small>{t(`${category.applicationCount} ${category.applicationCount === 1 ? "app" : "apps"}`)}</small>
            </button>
          ))}
        </section>
      )}

      <div className="applications-layout">
        <section className="applications-list-card" aria-label={t("Applications ranked by usage")}>
          <div className="list-heading">
            <div>
              <p className="section-kicker">{t("Active time")}</p>
              <h2>{t("Usage by application")}</h2>
            </div>
            <span>{t(`${visibleApplications.length} apps`)}</span>
          </div>

          {loading && applications.length === 0 && (
            <div className="applications-loading">
              <div className="skeleton app-row-skeleton" />
              <div className="skeleton app-row-skeleton" />
              <div className="skeleton app-row-skeleton short" />
            </div>
          )}

          {!loading && visibleApplications.length === 0 && !error && (
            <div className="empty-applications">
              <span>{t("0h")}</span>
              <h2>{t("No application activity")}</h2>
              <p>{t("Active applications will appear here as Watchhouse records them.")}</p>
            </div>
          )}

          {visibleApplications.map((application) => (
            <ApplicationRow
              application={application}
              total={total}
              selected={selected?.applicationId === application.applicationId}
              onSelect={() => setSelectedId(application.applicationId)}
              key={application.applicationId}
            />
          ))}
        </section>

        <aside className="application-detail">
          {selected ? (
            <>
              <ApplicationIcon
                className="detail-app-icon"
                applicationId={selected.applicationId}
                applicationName={selected.applicationName}
                style={{ "--app-hue": appHue(selected.applicationId) } as CSSProperties}
              />
              <p className="section-kicker">{t("Application")}</p>
              <h2>{selected.applicationName}</h2>
              <p className="detail-bundle">
                {selected.bundleIdentifier ?? t("No bundle identifier")}
              </p>
              <div className="application-preferences">
                <label>
                  {t("Category")}
                  <input
                    type="text"
                    list="application-categories"
                    maxLength={40}
                    value={categoryDraft}
                    disabled={savingPreferences}
                    onChange={(event) => setCategoryDraft(event.currentTarget.value)}
                    onBlur={() => {
                      if (categoryDraft.trim() && categoryDraft.trim() !== selected.category) {
                        void savePreferences(categoryDraft.trim(), selected.isIgnored);
                      }
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") event.currentTarget.blur();
                    }}
                  />
                  <datalist id="application-categories">
                    {["Uncategorized", "Work", "Communication", "Learning", "Creative", "Entertainment"].map((category) => (
                      <option value={category} key={category} />
                    ))}
                  </datalist>
                </label>
                <label className="ignore-application">
                  <input
                    type="checkbox"
                    checked={selected.isIgnored}
                    disabled={savingPreferences}
                    onChange={(event) => {
                      void savePreferences(selected.category, event.currentTarget.checked);
                    }}
                  />
                  <span>
                    <strong>{t("Ignore future activity")}</strong>
                    <small>{t("Existing history is preserved.")}</small>
                  </span>
                </label>
                <label className="ignore-application">
                  <input
                    type="checkbox"
                    checked={selected.recordWindowTitles}
                    disabled={savingPreferences}
                    onChange={(event) => {
                      void savePreferences(
                        selected.category,
                        selected.isIgnored,
                        event.currentTarget.checked,
                      );
                    }}
                  />
                  <span>
                    <strong>{t("Record window titles")}</strong>
                    <small>{t("Used only when the global privacy setting is enabled.")}</small>
                  </span>
                </label>
                {preferenceError && <small className="preference-error">{preferenceError}</small>}
              </div>
              <dl>
                <div>
                  <dt>{t("Active time")}</dt>
                  <dd>{formatDuration(selected.durationMs, locale)}</dd>
                </div>
                <div>
                  <dt>{t("Share of time")}</dt>
                  <dd>{total > 0 ? ((selected.durationMs / total) * 100).toFixed(1) : "0"}%</dd>
                </div>
                <div>
                  <dt>{t("Daily average")}</dt>
                  <dd>{formatDuration(selected.durationMs / rangeDays, locale)}</dd>
                </div>
              </dl>
              <div className="application-trend">
                <div>
                  <p className="section-kicker">{t("Daily trend")}</p>
                  <strong>{t(visibleTrend.length > 1 ? `Last ${visibleTrend.length} active days` : "Selected range")}</strong>
                </div>
                {visibleTrend.length ? (
                  <div className="application-trend-bars">
                    {visibleTrend.map((day) => (
                      <span key={day.date} title={`${day.date} · ${formatDuration(day.activeDurationMs, locale)}`}>
                        <i style={{ height: `${Math.max(5, day.activeDurationMs / trendMaximum * 100)}%` }} />
                      </span>
                    ))}
                  </div>
                ) : (
                  <small>{t("No daily trend in this range.")}</small>
                )}
              </div>
            </>
          ) : (
            <div className="detail-placeholder">
              <p>{t("Application details will appear here.")}</p>
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}
