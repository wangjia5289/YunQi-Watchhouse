import { useMemo, useState } from "react";
import {
  dateFromLocalIso,
  formatClock,
  formatDuration,
  localIsoDate,
  shiftLocalDate,
} from "../../lib/format";
import {
  ActivityState,
  TimelineEntry,
  deleteTimelineSession,
  errorMessage,
  updateTimelineSession,
} from "../../lib/ipc";
import { notifyActivityDataChanged } from "../../lib/events";
import { useTimeline } from "./useTimeline";
import { ApplicationIcon } from "../applications/ApplicationIcon";

function DayButton({
  direction,
  disabled,
  onClick,
}: {
  direction: "previous" | "next";
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className="day-arrow"
      type="button"
      aria-label={`${direction} day`}
      disabled={disabled}
      onClick={onClick}
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d={direction === "previous" ? "m15 5-7 7 7 7" : "m9 5 7 7-7 7"} />
      </svg>
    </button>
  );
}

function stateTotal(
  entries: { state: ActivityState; durationMs: number }[],
  state: ActivityState,
): number {
  return entries
    .filter((entry) => entry.state === state)
    .reduce((total, entry) => total + entry.durationMs, 0);
}

interface HourApplication {
  id: number;
  name: string;
  durationMs: number;
}

interface HourSummary {
  startedAtMs: number;
  activeDurationMs: number;
  idleDurationMs: number;
  sessionCount: number;
  applications: HourApplication[];
}

function summarizeByHour(entries: TimelineEntry[]): HourSummary[] {
  const groups = new Map<number, {
    activeDurationMs: number;
    idleDurationMs: number;
    sessionIds: Set<number>;
    applications: Map<number, HourApplication>;
  }>();

  for (const entry of entries) {
    let cursor = entry.startedAtMs;
    while (cursor < entry.endedAtMs) {
      const hour = new Date(cursor);
      hour.setMinutes(0, 0, 0);
      const hourStart = hour.getTime();
      const segmentEnd = Math.min(entry.endedAtMs, hourStart + 60 * 60 * 1_000);
      const durationMs = Math.max(0, segmentEnd - cursor);
      const group = groups.get(hourStart) ?? {
        activeDurationMs: 0,
        idleDurationMs: 0,
        sessionIds: new Set<number>(),
        applications: new Map<number, HourApplication>(),
      };
      group.sessionIds.add(entry.sessionId);
      if (entry.state === "IDLE") {
        group.idleDurationMs += durationMs;
      } else {
        group.activeDurationMs += durationMs;
        if (entry.applicationId !== null) {
          const application = group.applications.get(entry.applicationId) ?? {
            id: entry.applicationId,
            name: entry.applicationName ?? "Unknown application",
            durationMs: 0,
          };
          application.durationMs += durationMs;
          group.applications.set(entry.applicationId, application);
        }
      }
      groups.set(hourStart, group);
      cursor = segmentEnd;
    }
  }

  return [...groups.entries()]
    .sort(([left], [right]) => left - right)
    .map(([startedAtMs, group]) => ({
      startedAtMs,
      activeDurationMs: group.activeDurationMs,
      idleDurationMs: group.idleDurationMs,
      sessionCount: group.sessionIds.size,
      applications: [...group.applications.values()]
        .sort((left, right) => right.durationMs - left.durationMs)
        .slice(0, 3),
    }));
}

export function Timeline() {
  const today = localIsoDate();
  const [date, setDate] = useState(today);
  const [view, setView] = useState<"overview" | "details">("overview");
  const { entries, loading, error, refresh } = useTimeline(date);
  const [query, setQuery] = useState("");
  const [stateFilter, setStateFilter] = useState<"ALL" | ActivityState>("ALL");
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editStart, setEditStart] = useState("");
  const [editEnd, setEditEnd] = useState("");
  const [editError, setEditError] = useState<string | null>(null);
  const filteredEntries = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return entries.filter((entry) => {
      if (stateFilter !== "ALL" && entry.state !== stateFilter) return false;
      if (!normalized) return true;
      return [entry.applicationName, entry.bundleIdentifier, entry.category]
        .filter(Boolean)
        .some((value) => value!.toLocaleLowerCase().includes(normalized));
    });
  }, [entries, query, stateFilter]);
  const hours = useMemo(() => summarizeByHour(filteredEntries), [filteredEntries]);
  const dateValue = dateFromLocalIso(date);
  const isToday = date === today;
  const activeTotal = stateTotal(filteredEntries, "ACTIVE");
  const idleTotal = stateTotal(filteredEntries, "IDLE");
  const dateTitle = isToday
    ? "Today"
    : new Intl.DateTimeFormat(undefined, {
        weekday: "long",
        month: "long",
        day: "numeric",
      }).format(dateValue);

  return (
    <div className="timeline-page">
      <header className="timeline-header">
        <div>
          <p className="date-label">Computer activity</p>
          <h1>Timeline</h1>
        </div>
        <div className="date-controls">
          <DayButton
            direction="previous"
            onClick={() => setDate(shiftLocalDate(date, -1))}
          />
          <label className="date-picker">
            <span>{dateTitle}</span>
            <input
              type="date"
              value={date}
              max={today}
              onChange={(event) => {
                if (event.currentTarget.value) setDate(event.currentTarget.value);
              }}
              aria-label="Timeline date"
            />
          </label>
          <DayButton
            direction="next"
            disabled={isToday}
            onClick={() => setDate(shiftLocalDate(date, 1))}
          />
          {!isToday && (
            <button className="today-button" type="button" onClick={() => setDate(today)}>
              Today
            </button>
          )}
        </div>
      </header>

      <section className="timeline-summary" aria-label="Selected day summary">
        <div>
          <span className="summary-swatch active" />
          <p>Active</p>
          <strong>{formatDuration(activeTotal)}</strong>
        </div>
        <div>
          <span className="summary-swatch idle" />
          <p>Idle</p>
          <strong>{formatDuration(idleTotal)}</strong>
        </div>
        <div>
          <span className="summary-swatch sessions" />
          <p>Sessions</p>
          <strong>{filteredEntries.length}</strong>
        </div>
      </section>

      <div className="timeline-view-bar">
        <div>
          <p className="section-kicker">Day structure</p>
          <strong>{view === "overview" ? `${hours.length} active time blocks` : `${filteredEntries.length} matching sessions`}</strong>
        </div>
        <div className="range-tabs" role="group" aria-label="Timeline view">
          <button className={view === "overview" ? "active" : ""} onClick={() => setView("overview")}>
            Overview
          </button>
          <button className={view === "details" ? "active" : ""} onClick={() => setView("details")}>
            Details
          </button>
        </div>
      </div>

      <div className="timeline-filters">
        <label>
          <span>Search</span>
          <input
            type="search"
            value={query}
            placeholder="Application, bundle, or category"
            onChange={(event) => setQuery(event.currentTarget.value)}
          />
        </label>
        <label>
          <span>State</span>
          <select
            value={stateFilter}
            onChange={(event) => setStateFilter(event.currentTarget.value as "ALL" | ActivityState)}
          >
            <option value="ALL">All activity</option>
            <option value="ACTIVE">Active</option>
            <option value="IDLE">Idle</option>
          </select>
        </label>
        {(query || stateFilter !== "ALL") && (
          <button type="button" onClick={() => {
            setQuery("");
            setStateFilter("ALL");
          }}>Clear</button>
        )}
      </div>
      {editError && <div className="error-banner" role="alert">{editError}</div>}

      {error && (
        <div className="error-banner" role="alert">
          <span>{error}</span>
          <button type="button" onClick={refresh}>
            Try again
          </button>
        </div>
      )}

      {view === "overview" ? (
        <section className="timeline-overview" aria-label={`${dateTitle} hourly overview`}>
          {loading && entries.length === 0 && (
            <div className="timeline-loading">
              <div className="skeleton timeline-skeleton" />
              <div className="skeleton timeline-skeleton short" />
            </div>
          )}
          {!loading && hours.length === 0 && !error && (
            <div className="empty-timeline">
              <h2>{entries.length ? "No matching activity" : "No activity recorded"}</h2>
              <p>{entries.length ? "Adjust or clear the filters to see other sessions." : "Watchhouse did not record any computer activity on this day."}</p>
            </div>
          )}
          {hours.map((hour) => {
            const recorded = hour.activeDurationMs + hour.idleDurationMs;
            const activeShare = recorded ? hour.activeDurationMs / recorded * 100 : 0;
            return (
              <article className="hour-block" key={hour.startedAtMs}>
                <time>{formatClock(hour.startedAtMs)}</time>
                <div className="hour-content">
                  <div className="hour-heading">
                    <div>
                      <strong>{formatDuration(hour.activeDurationMs)} active</strong>
                      <span>{formatDuration(hour.idleDurationMs)} idle · {hour.sessionCount} sessions</span>
                    </div>
                    <span>{Math.round(activeShare)}% active</span>
                  </div>
                  <div className="hour-track">
                    <i style={{ width: `${activeShare}%` }} />
                  </div>
                  <div className="hour-applications">
                    {hour.applications.length ? hour.applications.map((application) => (
                      <div className="hour-application" key={application.id}>
                        <ApplicationIcon
                          className="hour-app-icon"
                          applicationId={application.id}
                          applicationName={application.name}
                        />
                        <span>{application.name}</span>
                        <strong>{formatDuration(application.durationMs)}</strong>
                      </div>
                    )) : <small>No active application in this hour.</small>}
                  </div>
                </div>
              </article>
            );
          })}
        </section>
      ) : (
      <section className="timeline-list" aria-label={`${dateTitle} activity sessions`}>
        {loading && entries.length === 0 && (
          <div className="timeline-loading">
            <div className="skeleton timeline-skeleton" />
            <div className="skeleton timeline-skeleton short" />
            <div className="skeleton timeline-skeleton" />
          </div>
        )}

        {!loading && filteredEntries.length === 0 && !error && (
          <div className="empty-timeline">
            <span className="empty-clock">
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <circle cx="12" cy="12" r="8" />
                <path d="M12 7v5l3 2" />
              </svg>
            </span>
            <h2>{entries.length ? "No matching activity" : "No activity recorded"}</h2>
            <p>{entries.length ? "Adjust or clear the filters to see other sessions." : "Watchhouse did not record any computer activity on this day."}</p>
          </div>
        )}

        {filteredEntries.map((entry, index) => {
          const idle = entry.state === "IDLE";
          const name = idle ? "Idle" : entry.applicationName ?? "Unknown application";
          return (
            <article className={`timeline-row${idle ? " idle" : ""}`} key={entry.sessionId}>
              <time>{formatClock(entry.startedAtMs)}</time>
              <div className="timeline-rail" aria-hidden="true">
                <span />
                {index < filteredEntries.length - 1 && <i />}
              </div>
              <div className="session-card">
                {idle || entry.applicationId === null ? (
                  <span className="app-badge" aria-hidden="true">
                    <svg viewBox="0 0 24 24">
                      <path d="M7 15c1.2 2 3 3 5 3 3.3 0 6-2.7 6-6 0-2-.9-3.8-2.4-4.9.3.7.4 1.3.4 1.9a5 5 0 0 1-9 3z" />
                    </svg>
                  </span>
                ) : (
                  <ApplicationIcon
                    className="app-badge timeline-app-icon"
                    applicationId={entry.applicationId}
                    applicationName={name}
                  />
                )}
                <div className="session-copy">
                  <strong>{name}</strong>
                  <span>
                    {formatClock(entry.startedAtMs)} – {formatClock(entry.endedAtMs)}
                    {entry.isOpen && <i className="open-session">Live</i>}
                  </span>
                </div>
                {editingId === entry.sessionId && (
                  <div className="session-editor">
                    <input
                      type="datetime-local"
                      value={editStart}
                      onChange={(event) => setEditStart(event.currentTarget.value)}
                      aria-label="Session start"
                    />
                    <input
                      type="datetime-local"
                      value={editEnd}
                      onChange={(event) => setEditEnd(event.currentTarget.value)}
                      aria-label="Session end"
                    />
                    <button type="button" onClick={() => {
                      const startedAtMs = new Date(editStart).getTime();
                      const endedAtMs = new Date(editEnd).getTime();
                      if (!Number.isFinite(startedAtMs) || endedAtMs <= startedAtMs) {
                        setEditError("Session end must be after its start.");
                        return;
                      }
                      void updateTimelineSession(entry.sessionId, startedAtMs, endedAtMs)
                        .then(() => {
                          setEditingId(null);
                          setEditError(null);
                          notifyActivityDataChanged();
                          refresh();
                        })
                        .catch((reason) => setEditError(errorMessage(reason)));
                    }}>Save</button>
                    <button type="button" onClick={() => setEditingId(null)}>Cancel</button>
                  </div>
                )}
                <span className="session-duration">{formatDuration(entry.durationMs)}</span>
                {!entry.isOpen && (
                  <>
                  <button
                    type="button"
                    className="edit-session"
                    aria-label={`Edit ${name} session`}
                    title="Edit session time"
                    onClick={() => {
                      setEditingId(entry.sessionId);
                      setEditStart(toLocalDateTimeInput(entry.startedAtMs));
                      setEditEnd(toLocalDateTimeInput(entry.endedAtMs));
                      setEditError(null);
                    }}
                  >Edit</button>
                  <button
                    type="button"
                    className="delete-session"
                    aria-label={`Delete ${name} session at ${formatClock(entry.startedAtMs)}`}
                    title="Delete session"
                    onClick={() => {
                      if (window.confirm(`Delete this ${name} session? This cannot be undone.`)) {
                        void deleteTimelineSession(entry.sessionId).then(refresh);
                      }
                    }}
                  >
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                      <path d="M5 7h14M9 7V4h6v3M8 10v8M12 10v8M16 10v8M7 7l1 14h8l1-14" />
                    </svg>
                  </button>
                  </>
                )}
              </div>
            </article>
          );
        })}

        {filteredEntries.length > 0 && (
          <div className="timeline-end">
            <time>
              {formatClock(filteredEntries[filteredEntries.length - 1]?.endedAtMs ?? null)}
            </time>
            <span />
            <p>End of recorded activity</p>
          </div>
        )}
      </section>
      )}
    </div>
  );
}

function toLocalDateTimeInput(timestamp: number): string {
  const date = new Date(timestamp);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(timestamp - offset).toISOString().slice(0, 16);
}
