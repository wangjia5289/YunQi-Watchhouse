import { CSSProperties, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { formatClock, formatDuration, formatLongDate } from "../../lib/format";
import {
  FocusModeStatus,
  FocusPlanTemplate,
  TimelineEntry,
  UsageLimitProgress,
  createFocusPlanTemplate,
  deleteFocusPlanTemplate,
  endFocusPlan,
  errorMessage,
  getFocusMode,
  getFocusPlanTemplates,
  setFocusPlanPaused,
  startFocusPlan,
  startFocusTemplate,
  setTrackingPaused,
  updateFocusPlanTemplate,
} from "../../lib/ipc";
import { useDashboard } from "./useDashboard";
import { useLocale } from "../../lib/i18n";
import "./DashboardAnime.css";

function UsageLimitSummary({ progress }: { progress: UsageLimitProgress[] }) {
  const { locale, t } = useLocale();
  const enabled = progress
    .filter((item) => item.enabled)
    .sort((left, right) => {
      const thresholdRank = {
        BELOW_80: 0,
        REACHED_80: 1,
        REACHED_100: 2,
      };
      return thresholdRank[right.thresholdState] - thresholdRank[left.thresholdState]
        || right.percentage - left.percentage;
    });

  if (enabled.length === 0) return null;

  return (
    <section className="usage-limit-summary" aria-labelledby="usage-limit-heading">
      <div className="usage-limit-heading">
        <div>
          <p className="section-kicker">{t("Limits")}</p>
          <h2 id="usage-limit-heading">{t("Today's usage limits")}</h2>
        </div>
        {enabled.length > 3 && (
          <span>{t("Showing the three closest limits")}</span>
        )}
      </div>
      <div className="usage-limit-list">
        {enabled.slice(0, 3).map((item) => {
          const label = item.scopeType === "APPLICATION"
            ? item.applicationName ?? t("Application")
            : item.category ?? t("Category");
          const limitDurationMs = item.limitMinutes * 60_000;
          const remainingDurationMs = Math.max(0, limitDurationMs - item.usedDurationMs);
          const percentage = Math.max(0, item.percentage);
          const progressWidth = Math.min(100, percentage);
          const stateClass = item.thresholdState === "REACHED_100"
            ? " reached"
            : item.thresholdState === "REACHED_80"
              ? " approaching"
              : "";

          return (
            <article className={`usage-limit-item${stateClass}`} key={item.id}>
              <div className="usage-limit-title">
                <strong title={label}>{label}</strong>
                <span>
                  {item.thresholdState === "REACHED_100"
                    ? t("Limit reached")
                    : item.thresholdState === "REACHED_80"
                      ? t("Approaching limit")
                      : `${Math.round(percentage)}%`}
                </span>
              </div>
              <div
                className="usage-limit-track"
                role="progressbar"
                aria-label={label}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(progressWidth)}
              >
                <i style={{ "--limit-progress": `${progressWidth}%` } as CSSProperties} />
              </div>
              <div className="usage-limit-metrics">
                <span>
                  {t("Used")}
                  <strong>{formatDuration(item.usedDurationMs, locale)}</strong>
                </span>
                <span>
                  {t("Limit")}
                  <strong>{formatDuration(limitDurationMs, locale)}</strong>
                </span>
                <span>
                  {t("Remaining")}
                  <strong>
                    {item.thresholdState === "REACHED_100"
                      ? t("None")
                      : formatDuration(remainingDurationMs, locale)}
                  </strong>
                </span>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}

function ActivityDistribution({
  timeline,
  startMs,
  endMs,
}: {
  timeline: TimelineEntry[];
  startMs: number;
  endMs: number;
}) {
  const { locale, t } = useLocale();
  const dayDuration = endMs - startMs;
  const segments = timeline
    .filter((entry) => entry.state === "ACTIVE" && entry.durationMs > 0)
    .map((entry) => {
      const left = ((entry.startedAtMs - startMs) / dayDuration) * 100;
      const width = (entry.durationMs / dayDuration) * 100;
      return {
        entry,
        style: {
          "--segment-left": `${Math.max(0, left)}%`,
          "--segment-width": `${Math.max(0.18, width)}%`,
        } as CSSProperties,
      };
    });

  return (
    <section className="activity-card" aria-labelledby="activity-heading">
      <div className="section-heading">
        <div>
          <p className="section-kicker">{t("Overview")}</p>
          <h2 id="activity-heading">{t("Today's activity")}</h2>
        </div>
        <span className="session-count">
          {t(`${timeline.filter((entry) => entry.state === "ACTIVE").length} sessions`)}
        </span>
      </div>

      <div className="time-scale" aria-hidden="true">
        {["12a", "3a", "6a", "9a", "12p", "3p", "6p", "9p", "12a"].map(
          (label, index) => (
            <span key={`${label}-${index}`}>{label}</span>
          ),
        )}
      </div>
      <div
        className="activity-track"
        role="img"
        aria-label={t(`${segments.length} active periods recorded today`)}
      >
        <div className="track-grid" aria-hidden="true" />
        {segments.map(({ entry, style }) => (
          <span
            className="activity-segment"
            style={style}
            key={entry.sessionId}
            title={`${entry.applicationName ?? t("Active")} · ${formatDuration(entry.durationMs, locale)}`}
          />
        ))}
      </div>
      <div className="activity-legend">
        <span>
          <i className="legend-dot active" /> {t("Active")}
        </span>
        <span>
          <i className="legend-dot quiet" /> {t("No activity recorded")}
        </span>
      </div>
    </section>
  );
}

export function Dashboard() {
  const { locale, t } = useLocale();
  const {
    summary,
    timeline,
    current,
    focus,
    usageLimits,
    loading,
    error,
    refresh,
  } = useDashboard();
  const [dismissedBlockStart, setDismissedBlockStart] = useState<number | null>(null);
  const [focusMode, setFocusModeStatus] = useState<FocusModeStatus | null>(null);
  const [focusPlanMinutes, setFocusPlanMinutes] = useState(50);
  const [focusTemplates, setFocusTemplates] = useState<FocusPlanTemplate[]>([]);
  const [templateName, setTemplateName] = useState("");
  const [templateMinutes, setTemplateMinutes] = useState(50);
  const [templateOpen, setTemplateOpen] = useState(false);
  const [editingTemplateId, setEditingTemplateId] = useState<number | null>(null);
  const [templateError, setTemplateError] = useState<string | null>(null);
  const [, setClockRevision] = useState(0);

  useEffect(() => {
    void getFocusMode().then(setFocusModeStatus);
    void getFocusPlanTemplates().then(setFocusTemplates);
    const unlisten = listen<FocusModeStatus>("focus-mode-changed", (event) => {
      setFocusModeStatus(event.payload);
    });
    const timer = window.setInterval(() => setClockRevision((value) => value + 1), 1_000);
    return () => {
      window.clearInterval(timer);
      void unlisten.then((stop) => stop());
    };
  }, []);
  const runningSample =
    current?.monitor.status === "RUNNING" ? current.monitor.payload : null;
  const degraded =
    current?.monitor.status === "DEGRADED" ||
    current?.persistence.status === "DEGRADED";
  const trackingLabel = degraded
    ? "Needs attention"
    : current?.paused
      ? "Tracking paused"
      : runningSample?.state === "IDLE"
      ? "Idle"
      : "Tracking";
  const currentApp =
    current?.paused
      ? "No new activity is being recorded"
      : runningSample?.state === "ACTIVE"
      ? runningSample.foregroundApplication?.name
      : runningSample?.state === "IDLE"
        ? "Away from computer"
        : "Starting activity monitor";
  const currentFocusBlock = focus?.blocks.find((block) => block.isOpen) ?? null;
  const focusGoalMs = (focus?.goalMinutes ?? 0) * 60_000;
  const focusProgress = focusGoalMs > 0
    ? Math.min(100, ((focus?.totalFocusDurationMs ?? 0) / focusGoalMs) * 100)
    : 0;
  const showBreakReminder = Boolean(
    focus?.breakRemindersEnabled
      && currentFocusBlock
      && currentFocusBlock.activeDurationMs >= focus.breakReminderMinutes * 60_000
      && dismissedBlockStart !== currentFocusBlock.startedAtMs
      && !isQuietHours(focus.quietHoursStart, focus.quietHoursEnd),
  );

  async function moveTemplate(index: number, delta: -1 | 1) {
    const targetIndex = index + delta;
    if (targetIndex < 0 || targetIndex >= focusTemplates.length) return;
    const current = focusTemplates[index];
    const target = focusTemplates[targetIndex];
    try {
      const [updatedCurrent, updatedTarget] = await Promise.all([
        updateFocusPlanTemplate(
          current.id,
          current.name,
          current.durationMinutes,
          target.sortOrder,
        ),
        updateFocusPlanTemplate(
          target.id,
          target.name,
          target.durationMinutes,
          current.sortOrder,
        ),
      ]);
      setFocusTemplates((templates) => templates
        .map((template) => template.id === updatedCurrent.id
          ? updatedCurrent
          : template.id === updatedTarget.id
            ? updatedTarget
            : template)
        .sort((left, right) => left.sortOrder - right.sortOrder));
    } catch (reason) {
      setTemplateError(errorMessage(reason));
    }
  }

  return (
    <div className="dashboard anime-dashboard">
      <div className="anime-sky" aria-hidden="true">
        <span className="anime-sun" />
        <span className="anime-cloud cloud-one" />
        <span className="anime-cloud cloud-two" />
        <span className="anime-star star-one" />
        <span className="anime-star star-two" />
        <span className="anime-star star-three" />
      </div>
      <header className="topbar">
        <div>
          <p className="date-label">{formatLongDate(new Date(), locale)}</p>
          <h1>{t("Today")}</h1>
        </div>
        <button
          type="button"
          className={`tracking-pill${degraded ? " degraded" : ""}${current?.paused ? " paused" : ""}`}
          onClick={() => void setTrackingPaused(!current?.paused).then(refresh)}
          title={t(current?.paused ? "Resume tracking" : "Pause tracking")}
        >
          <span className="tracking-dot" />
          <span>
            <strong>{t(trackingLabel)}</strong>
            <small>{currentApp ? t(currentApp) : "—"}</small>
          </span>
        </button>
      </header>

      {error && (
        <div className="error-banner" role="alert">
          <span>{t(error)}</span>
          <button type="button" onClick={refresh}>
            {t("Try again")}
          </button>
        </div>
      )}
      {templateError && <div className="error-banner" role="alert">{templateError}</div>}

      {showBreakReminder && currentFocusBlock && (
        <div className="break-reminder" role="status">
          <div>
            <strong>{t("Time for a short break")}</strong>
            <span>{t(`You have focused for ${formatDuration(currentFocusBlock.activeDurationMs, locale)}.`)}</span>
          </div>
          <button type="button" onClick={() => setDismissedBlockStart(currentFocusBlock.startedAtMs)}>
            {t("Dismiss")}
          </button>
        </div>
      )}

      <section className="summary-grid" aria-label={t("Today summary")}>
        <article className="primary-stat">
          <p>{t("Active time")}</p>
          <strong>{loading ? "—" : formatDuration(summary?.activeDurationMs ?? 0, locale)}</strong>
          <span>
            <i className="live-indicator" />
            {t("Recorded locally on this Mac")}
          </span>
        </article>

        <div className="secondary-stats">
          <article>
            <p>{t("First activity")}</p>
            <strong>{formatClock(summary?.firstActivityAtMs ?? null, locale)}</strong>
            <span>{t("Started today")}</span>
          </article>
          <article>
            <p>{t("Last activity")}</p>
            <strong>{formatClock(summary?.lastActivityAtMs ?? null, locale)}</strong>
            <span>{t("Latest checkpoint")}</span>
          </article>
          <article>
            <p>{t("Idle")}</p>
            <strong>{formatDuration(summary?.idleDurationMs ?? 0, locale)}</strong>
            <span>{t("Away from computer")}</span>
          </article>
        </div>
      </section>

      {focus && (
        <section className="focus-summary" aria-label={t("Focus summary")}>
          <div className="focus-progress">
            <span style={{ width: `${focusProgress}%` }} />
          </div>
          <article><span>{t("Focused today")}</span><strong>{formatDuration(focus.totalFocusDurationMs, locale)}</strong></article>
          <article><span>{t("Longest block")}</span><strong>{formatDuration(focus.longestFocusDurationMs, locale)}</strong></article>
          <article><span>{t("App switches")}</span><strong>{focus.applicationSwitchCount}</strong></article>
          <article>
            <span>{t("Daily goal")}</span>
            <strong>{focus.goalMinutes ? `${Math.round(focusProgress)}%` : t("Off")}</strong>
          </article>
          <div className="focus-plan-controls">
            {focusMode?.active ? (
              <>
                <div>
                  <strong>
                    {focusMode.paused
                      ? t("Focus paused")
                      : focusMode.plannedEndAtMs
                        ? t(`${formatDuration(Math.max(0, focusMode.plannedEndAtMs - Date.now()), locale)} remaining`)
                        : t(`Focused ${formatDuration(Date.now() - (focusMode.startedAtMs ?? Date.now()), locale)}`)}
                  </strong>
                  <small>
                    {focusMode.plannedEndAtMs
                      ? t(`Planned until ${formatClock(focusMode.plannedEndAtMs, locale)}`)
                      : t("Open-ended focus mode")}
                  </small>
                </div>
                <button
                  type="button"
                  onClick={() => void setFocusPlanPaused(!focusMode.paused).then(setFocusModeStatus)}
                >
                  {t(focusMode.paused ? "Resume" : "Pause")}
                </button>
                <button
                  type="button"
                  className="focus-plan-end"
                  onClick={() => void endFocusPlan(false).then(setFocusModeStatus)}
                >
                  {t("End")}
                </button>
              </>
            ) : (
              <>
                <div className="focus-template-list" aria-label={t("Focus plan templates")}>
                  {focusTemplates.map((template, index) => (
                    <span key={template.id}>
                      <button
                        type="button"
                        onClick={() => {
                          setTemplateError(null);
                          void startFocusTemplate(template.id)
                            .then(setFocusModeStatus)
                            .catch((reason) => setTemplateError(errorMessage(reason)));
                        }}
                      >
                        {template.name} · {t(`${template.durationMinutes}m`)}
                      </button>
                      <button
                        type="button"
                        aria-label={t(`Edit ${template.name} template`)}
                        onClick={() => {
                          setEditingTemplateId(template.id);
                          setTemplateName(template.name);
                          setTemplateMinutes(template.durationMinutes);
                          setTemplateOpen(true);
                        }}
                      >
                        {t("Edit")}
                      </button>
                      <button
                        type="button"
                        aria-label={t(`Move ${template.name} template up`)}
                        disabled={index === 0}
                        onClick={() => void moveTemplate(index, -1)}
                      >
                        {t("Up")}
                      </button>
                      <button
                        type="button"
                        aria-label={t(`Move ${template.name} template down`)}
                        disabled={index === focusTemplates.length - 1}
                        onClick={() => void moveTemplate(index, 1)}
                      >
                        {t("Down")}
                      </button>
                      <button
                        type="button"
                        aria-label={t(`Remove ${template.name} template`)}
                        onClick={() => {
                          setTemplateError(null);
                          void deleteFocusPlanTemplate(template.id).then(() => {
                            setFocusTemplates((current) => current.filter((item) => item.id !== template.id));
                          }).catch((reason) => setTemplateError(errorMessage(reason)));
                        }}
                      >
                        {t("Remove")}
                      </button>
                    </span>
                  ))}
                </div>
                {focusTemplates.length > 0 && (
                  <small className="focus-template-stats">
                    {focusTemplates.map((template) => {
                      const rate = template.useCount
                        ? Math.round(template.completedCount / template.useCount * 100)
                        : 0;
                      return t(`${template.name}: ${template.useCount} starts · ${rate}% complete`);
                    }).join(" · ")}
                  </small>
                )}
                <select
                  value={focusPlanMinutes}
                  aria-label={t("Focus plan duration")}
                  onChange={(event) => setFocusPlanMinutes(Number(event.currentTarget.value))}
                >
                  {[25, 45, 50, 60, 90, 120].map((minutes) => (
                    <option value={minutes} key={minutes}>{t(`${minutes} minutes`)}</option>
                  ))}
                </select>
                <button
                  type="button"
                  className="focus-mode-button"
                  onClick={() => void startFocusPlan(focusPlanMinutes).then(setFocusModeStatus)}
                >
                  {t("Start focus plan")}
                </button>
                <button type="button" onClick={() => setTemplateOpen((open) => !open)}>
                  {t(templateOpen ? "Close template editor" : "New template")}
                </button>
                {templateOpen && (
                  <div className="focus-template-editor">
                    <input
                      value={templateName}
                      maxLength={40}
                      placeholder={t("Template name")}
                      aria-label={t("Template name")}
                      onChange={(event) => setTemplateName(event.currentTarget.value)}
                    />
                    <input
                      type="number"
                      min={5}
                      max={240}
                      value={templateMinutes}
                      aria-label={t("Template duration in minutes")}
                      onChange={(event) => setTemplateMinutes(Number(event.currentTarget.value))}
                    />
                    <button
                      type="button"
                      disabled={!templateName.trim() || templateMinutes < 5 || templateMinutes > 240}
                      onClick={() => {
                        setTemplateError(null);
                        const request = editingTemplateId === null
                          ? createFocusPlanTemplate(templateName, templateMinutes)
                          : updateFocusPlanTemplate(
                              editingTemplateId,
                              templateName,
                              templateMinutes,
                              focusTemplates.find((item) => item.id === editingTemplateId)?.sortOrder ?? 0,
                            );
                        void request.then((template) => {
                            setFocusTemplates((current) => editingTemplateId === null
                              ? [...current, template]
                              : current.map((item) => item.id === template.id ? template : item));
                            setTemplateName("");
                            setEditingTemplateId(null);
                            setTemplateOpen(false);
                          })
                          .catch((reason) => setTemplateError(errorMessage(reason)));
                      }}
                    >
                      {t(editingTemplateId === null ? "Save template" : "Update template")}
                    </button>
                  </div>
                )}
              </>
            )}
          </div>
        </section>
      )}

      <UsageLimitSummary progress={usageLimits} />

      {summary && (
        <ActivityDistribution
          timeline={timeline}
          startMs={summary.range.startMs}
          endMs={summary.range.endMs}
        />
      )}

      {!summary && !error && (
        <section className="activity-card skeleton-card" aria-label={t("Loading activity")}>
          <div className="skeleton title" />
          <div className="skeleton chart" />
        </section>
      )}

      <footer className="privacy-note">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M7 10V7a5 5 0 0 1 10 0v3M5 10h14v11H5z" />
        </svg>
        {t("Activity stays private and is stored only on this Mac.")}
      </footer>
    </div>
  );
}

function isQuietHours(start: string, end: string): boolean {
  const now = new Date();
  const current = now.getHours() * 60 + now.getMinutes();
  const parse = (value: string) => {
    const [hours = 0, minutes = 0] = value.split(":").map(Number);
    return hours * 60 + minutes;
  };
  const startMinutes = parse(start);
  const endMinutes = parse(end);
  return startMinutes <= endMinutes
    ? current >= startMinutes && current < endMinutes
    : current >= startMinutes || current < endMinutes;
}
