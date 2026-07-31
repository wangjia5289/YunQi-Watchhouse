import { CSSProperties, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { formatClock, formatDuration, formatLongDate } from "../../lib/format";
import {
  FocusModeStatus,
  FocusPlanTemplate,
  TimelineEntry,
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

function ActivityDistribution({
  timeline,
  startMs,
  endMs,
}: {
  timeline: TimelineEntry[];
  startMs: number;
  endMs: number;
}) {
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
          <p className="section-kicker">Overview</p>
          <h2 id="activity-heading">Today&apos;s activity</h2>
        </div>
        <span className="session-count">
          {timeline.filter((entry) => entry.state === "ACTIVE").length} sessions
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
        aria-label={`${segments.length} active periods recorded today`}
      >
        <div className="track-grid" aria-hidden="true" />
        {segments.map(({ entry, style }) => (
          <span
            className="activity-segment"
            style={style}
            key={entry.sessionId}
            title={`${entry.applicationName ?? "Active"} · ${formatDuration(entry.durationMs)}`}
          />
        ))}
      </div>
      <div className="activity-legend">
        <span>
          <i className="legend-dot active" /> Active
        </span>
        <span>
          <i className="legend-dot quiet" /> No activity recorded
        </span>
      </div>
    </section>
  );
}

export function Dashboard() {
  const { summary, timeline, current, focus, loading, error, refresh } = useDashboard();
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
    <div className="dashboard">
      <header className="topbar">
        <div>
          <p className="date-label">{formatLongDate(new Date())}</p>
          <h1>Today</h1>
        </div>
        <button
          type="button"
          className={`tracking-pill${degraded ? " degraded" : ""}${current?.paused ? " paused" : ""}`}
          onClick={() => void setTrackingPaused(!current?.paused).then(refresh)}
          title={current?.paused ? "Resume tracking" : "Pause tracking"}
        >
          <span className="tracking-dot" />
          <span>
            <strong>{trackingLabel}</strong>
            <small>{currentApp}</small>
          </span>
        </button>
      </header>

      {error && (
        <div className="error-banner" role="alert">
          <span>{error}</span>
          <button type="button" onClick={refresh}>
            Try again
          </button>
        </div>
      )}
      {templateError && <div className="error-banner" role="alert">{templateError}</div>}

      {showBreakReminder && currentFocusBlock && (
        <div className="break-reminder" role="status">
          <div>
            <strong>Time for a short break</strong>
            <span>You have focused for {formatDuration(currentFocusBlock.activeDurationMs)}.</span>
          </div>
          <button type="button" onClick={() => setDismissedBlockStart(currentFocusBlock.startedAtMs)}>
            Dismiss
          </button>
        </div>
      )}

      <section className="summary-grid" aria-label="Today summary">
        <article className="primary-stat">
          <p>Active time</p>
          <strong>{loading ? "—" : formatDuration(summary?.activeDurationMs ?? 0)}</strong>
          <span>
            <i className="live-indicator" />
            Recorded locally on this Mac
          </span>
        </article>

        <div className="secondary-stats">
          <article>
            <p>First activity</p>
            <strong>{formatClock(summary?.firstActivityAtMs ?? null)}</strong>
            <span>Started today</span>
          </article>
          <article>
            <p>Last activity</p>
            <strong>{formatClock(summary?.lastActivityAtMs ?? null)}</strong>
            <span>Latest checkpoint</span>
          </article>
          <article>
            <p>Idle</p>
            <strong>{formatDuration(summary?.idleDurationMs ?? 0)}</strong>
            <span>Away from computer</span>
          </article>
        </div>
      </section>

      {focus && (
        <section className="focus-summary" aria-label="Focus summary">
          <div className="focus-progress">
            <span style={{ width: `${focusProgress}%` }} />
          </div>
          <article><span>Focused today</span><strong>{formatDuration(focus.totalFocusDurationMs)}</strong></article>
          <article><span>Longest block</span><strong>{formatDuration(focus.longestFocusDurationMs)}</strong></article>
          <article><span>App switches</span><strong>{focus.applicationSwitchCount}</strong></article>
          <article>
            <span>Daily goal</span>
            <strong>{focus.goalMinutes ? `${Math.round(focusProgress)}%` : "Off"}</strong>
          </article>
          <div className="focus-plan-controls">
            {focusMode?.active ? (
              <>
                <div>
                  <strong>
                    {focusMode.paused
                      ? "Focus paused"
                      : focusMode.plannedEndAtMs
                        ? `${formatDuration(Math.max(0, focusMode.plannedEndAtMs - Date.now()))} remaining`
                        : `Focused ${formatDuration(Date.now() - (focusMode.startedAtMs ?? Date.now()))}`}
                  </strong>
                  <small>
                    {focusMode.plannedEndAtMs
                      ? `Planned until ${formatClock(focusMode.plannedEndAtMs)}`
                      : "Open-ended focus mode"}
                  </small>
                </div>
                <button
                  type="button"
                  onClick={() => void setFocusPlanPaused(!focusMode.paused).then(setFocusModeStatus)}
                >
                  {focusMode.paused ? "Resume" : "Pause"}
                </button>
                <button
                  type="button"
                  className="focus-plan-end"
                  onClick={() => void endFocusPlan(false).then(setFocusModeStatus)}
                >
                  End
                </button>
              </>
            ) : (
              <>
                <div className="focus-template-list" aria-label="Focus plan templates">
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
                        {template.name} · {template.durationMinutes}m
                      </button>
                      <button
                        type="button"
                        aria-label={`Edit ${template.name} template`}
                        onClick={() => {
                          setEditingTemplateId(template.id);
                          setTemplateName(template.name);
                          setTemplateMinutes(template.durationMinutes);
                          setTemplateOpen(true);
                        }}
                      >
                        Edit
                      </button>
                      <button
                        type="button"
                        aria-label={`Move ${template.name} template up`}
                        disabled={index === 0}
                        onClick={() => void moveTemplate(index, -1)}
                      >
                        Up
                      </button>
                      <button
                        type="button"
                        aria-label={`Move ${template.name} template down`}
                        disabled={index === focusTemplates.length - 1}
                        onClick={() => void moveTemplate(index, 1)}
                      >
                        Down
                      </button>
                      <button
                        type="button"
                        aria-label={`Remove ${template.name} template`}
                        onClick={() => {
                          setTemplateError(null);
                          void deleteFocusPlanTemplate(template.id).then(() => {
                            setFocusTemplates((current) => current.filter((item) => item.id !== template.id));
                          }).catch((reason) => setTemplateError(errorMessage(reason)));
                        }}
                      >
                        Remove
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
                      return `${template.name}: ${template.useCount} starts · ${rate}% complete`;
                    }).join(" · ")}
                  </small>
                )}
                <select
                  value={focusPlanMinutes}
                  aria-label="Focus plan duration"
                  onChange={(event) => setFocusPlanMinutes(Number(event.currentTarget.value))}
                >
                  {[25, 45, 50, 60, 90, 120].map((minutes) => (
                    <option value={minutes} key={minutes}>{minutes} minutes</option>
                  ))}
                </select>
                <button
                  type="button"
                  className="focus-mode-button"
                  onClick={() => void startFocusPlan(focusPlanMinutes).then(setFocusModeStatus)}
                >
                  Start focus plan
                </button>
                <button type="button" onClick={() => setTemplateOpen((open) => !open)}>
                  {templateOpen ? "Close template editor" : "New template"}
                </button>
                {templateOpen && (
                  <div className="focus-template-editor">
                    <input
                      value={templateName}
                      maxLength={40}
                      placeholder="Template name"
                      aria-label="Template name"
                      onChange={(event) => setTemplateName(event.currentTarget.value)}
                    />
                    <input
                      type="number"
                      min={5}
                      max={240}
                      value={templateMinutes}
                      aria-label="Template duration in minutes"
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
                      {editingTemplateId === null ? "Save template" : "Update template"}
                    </button>
                  </div>
                )}
              </>
            )}
          </div>
        </section>
      )}

      {summary && (
        <ActivityDistribution
          timeline={timeline}
          startMs={summary.range.startMs}
          endMs={summary.range.endMs}
        />
      )}

      {!summary && !error && (
        <section className="activity-card skeleton-card" aria-label="Loading activity">
          <div className="skeleton title" />
          <div className="skeleton chart" />
        </section>
      )}

      <footer className="privacy-note">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M7 10V7a5 5 0 0 1 10 0v3M5 10h14v11H5z" />
        </svg>
        Activity stays private and is stored only on this Mac.
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
