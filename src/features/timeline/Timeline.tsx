import { useEffect, useMemo, useState } from "react";
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
  deleteTimelineSessions,
  errorMessage,
  getTimelineUndoTokens,
  importActivity,
  ImportPreview,
  mergeTimelineSessions,
  previewActivityImport,
  splitTimelineSession,
  undoTimelineEdit,
  updateTimelineSessionCategories,
  updateTimelineSessionNotes,
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

type TimelineActionDialog = {
  kind: "note" | "category" | "delete";
  sessionIds: number[];
  label: string;
};

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
  const [splittingId, setSplittingId] = useState<number | null>(null);
  const [splitAt, setSplitAt] = useState("");
  const [editStart, setEditStart] = useState("");
  const [editEnd, setEditEnd] = useState("");
  const [editError, setEditError] = useState<string | null>(null);
  const [visibleCount, setVisibleCount] = useState(200);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [undoStack, setUndoStack] = useState<string[]>([]);
  const [operationMessage, setOperationMessage] = useState<string | null>(null);
  const [actionDialog, setActionDialog] = useState<TimelineActionDialog | null>(null);
  const [dialogValue, setDialogValue] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [importContents, setImportContents] = useState("");
  const [importFormat, setImportFormat] = useState<"json" | "csv">("json");
  const [importPreview, setImportPreview] = useState<ImportPreview | null>(null);
  const [conflictPolicy, setConflictPolicy] = useState<"skip" | "merge">("skip");
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
  const visibleEntries = filteredEntries.slice(0, visibleCount);

  useEffect(() => {
    setVisibleCount(200);
    setSelectedIds(new Set());
  }, [date, query, stateFilter]);
  useEffect(() => {
    void getTimelineUndoTokens().then(setUndoStack).catch(() => {});
  }, []);
  const selected = [...selectedIds];

  const completeMutation = (message: string, token?: string | null) => {
    setSelectedIds(new Set());
    if (token) setUndoStack((current) => [...current, token]);
    setOperationMessage(message);
    notifyActivityDataChanged();
    refresh();
  };

  const runOperation = async (operation: () => Promise<void>) => {
    setEditError(null);
    try {
      await operation();
    } catch (reason) {
      setEditError(errorMessage(reason));
    }
  };

  const submitActionDialog = () => {
    if (!actionDialog) return;
    const { kind, sessionIds } = actionDialog;
    if (kind === "category" && !dialogValue.trim()) {
      setEditError("Enter an application category.");
      return;
    }
    setActionDialog(null);
    void runOperation(async () => {
      if (kind === "note") {
        const count = await updateTimelineSessionNotes(sessionIds, dialogValue.trim() || null);
        completeMutation(`Updated notes on ${count} sessions.`);
      } else if (kind === "category") {
        const count = await updateTimelineSessionCategories(sessionIds, dialogValue.trim());
        completeMutation(`Updated ${count} application categories.`);
      } else {
        const result = sessionIds.length === 1
          ? await deleteTimelineSession(sessionIds[0])
          : await deleteTimelineSessions(sessionIds);
        completeMutation(`Deleted ${result.affectedCount} sessions.`, result.undoToken);
      }
    });
  };
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
        <button className="timeline-import-button" type="button" onClick={() => setImportOpen((open) => !open)}>
          Import
        </button>
      </div>

      {importOpen && (
        <section className="timeline-import" aria-label="Import activity">
          <div>
            <strong>Import Watchhouse data</strong>
            <span>Choose a JSON or CSV export. Nothing is written until you confirm.</span>
          </div>
          <input
            type="file"
            accept=".json,.csv,application/json,text/csv"
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              if (!file) return;
              const format = file.name.toLowerCase().endsWith(".csv") ? "csv" : "json";
              setImportFormat(format);
              setImportPreview(null);
              void file.text().then((contents) => {
                setImportContents(contents);
                return previewActivityImport(contents, format);
              }).then(setImportPreview).catch((reason) => setEditError(errorMessage(reason)));
            }}
          />
          {importPreview && (
            <div className="import-preview">
              <span>{importPreview.recordCount} valid sessions</span>
              <span>{importPreview.conflictCount} conflicts</span>
              <span>{importPreview.invalidCount} invalid</span>
              {importPreview.startedAtMs !== null && importPreview.endedAtMs !== null && (
                <span>{new Date(importPreview.startedAtMs).toLocaleDateString()} – {new Date(importPreview.endedAtMs).toLocaleDateString()}</span>
              )}
              <select value={conflictPolicy} onChange={(event) => setConflictPolicy(event.currentTarget.value as "skip" | "merge")} aria-label="Import conflict policy">
                <option value="skip">Skip conflicts</option>
                <option value="merge">Merge compatible conflicts</option>
              </select>
              <button
                type="button"
                disabled={importPreview.recordCount === 0 || importPreview.invalidCount > 0}
                onClick={() => void runOperation(async () => {
                  const result = await importActivity(importContents, importFormat, conflictPolicy);
                  setImportOpen(false);
                  setImportPreview(null);
                  completeMutation(`Imported ${result.importedCount}; merged ${result.mergedCount}; skipped ${result.skippedCount}.`);
                })}
              >Import activity</button>
            </div>
          )}
        </section>
      )}

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
      {operationMessage && (
        <div className="timeline-operation-message" role="status">
          <span>{operationMessage}</span>
          {undoStack.length > 0 && <button type="button" onClick={() => void runOperation(async () => {
            const undoToken = undoStack[undoStack.length - 1];
            const restored = await undoTimelineEdit(undoToken);
            setUndoStack((current) => current.slice(0, -1));
            completeMutation(`Restored ${restored} sessions. ${undoStack.length - 1} undo steps remain.`);
          })}>Undo ({undoStack.length})</button>}
          <button type="button" aria-label="Dismiss message" onClick={() => {
            setOperationMessage(null);
          }}>Close</button>
        </div>
      )}

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
      <>
      {filteredEntries.some((entry) => !entry.isOpen) && (
        <div className="timeline-selection-bar">
          <label>
            <input
              type="checkbox"
              checked={selected.length > 0 && selected.length === filteredEntries.filter((entry) => !entry.isOpen).length}
              onChange={(event) => setSelectedIds(event.currentTarget.checked
                ? new Set(filteredEntries.filter((entry) => !entry.isOpen).map((entry) => entry.sessionId))
                : new Set())}
            />
            {selected.length ? `${selected.length} selected` : "Select sessions"}
          </label>
          {selected.length > 0 && (
            <div>
              <button type="button" disabled={selected.length < 2} onClick={() => void runOperation(async () => {
                const result = await mergeTimelineSessions(selected);
                completeMutation(`Merged ${result.affectedCount} sessions.`, result.undoToken);
              })}>Merge</button>
              <button type="button" onClick={() => {
                setDialogValue("");
                setActionDialog({
                  kind: "note",
                  sessionIds: selected,
                  label: `${selected.length} selected sessions`,
                });
              }}>Note</button>
              <button type="button" onClick={() => {
                setDialogValue("");
                setActionDialog({
                  kind: "category",
                  sessionIds: selected,
                  label: `${selected.length} selected sessions`,
                });
              }}>Category</button>
              <button className="danger" type="button" onClick={() => {
                setActionDialog({
                  kind: "delete",
                  sessionIds: selected,
                  label: `${selected.length} selected sessions`,
                });
              }}>Delete</button>
            </div>
          )}
        </div>
      )}
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

        {visibleEntries.map((entry, index) => {
          const idle = entry.state === "IDLE";
          const name = idle ? "Idle" : entry.applicationName ?? "Unknown application";
          return (
            <article className={`timeline-row${idle ? " idle" : ""}`} key={entry.sessionId}>
              {!entry.isOpen && (
                <input
                  className="session-select"
                  type="checkbox"
                  checked={selectedIds.has(entry.sessionId)}
                  aria-label={`Select ${name} session`}
                  onChange={(event) => setSelectedIds((current) => {
                    const next = new Set(current);
                    if (event.currentTarget.checked) next.add(entry.sessionId);
                    else next.delete(entry.sessionId);
                    return next;
                  })}
                />
              )}
              <time>{formatClock(entry.startedAtMs)}</time>
              <div className="timeline-rail" aria-hidden="true">
                <span />
                {index < visibleEntries.length - 1 && <i />}
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
                  {entry.windowTitle && (
                    <small className="session-note">{entry.windowTitle}</small>
                  )}
                  {entry.note && <small className="session-note">{entry.note}</small>}
                </div>
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
                    className="edit-session"
                    aria-label={`Split ${name} session`}
                    title="Split session"
                    onClick={() => {
                      setSplittingId(entry.sessionId);
                      setSplitAt(toLocalDateTimeInput(
                        entry.startedAtMs + Math.floor(entry.durationMs / 2),
                      ));
                      setEditError(null);
                    }}
                  >Split</button>
                  <button
                    type="button"
                    className="delete-session"
                    aria-label={`Delete ${name} session at ${formatClock(entry.startedAtMs)}`}
                    title="Delete session"
                    onClick={() => {
                      setActionDialog({
                        kind: "delete",
                        sessionIds: [entry.sessionId],
                        label: `${name}, ${formatClock(entry.startedAtMs)}–${formatClock(entry.endedAtMs)}`,
                      });
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

        {visibleCount < filteredEntries.length && (
          <button
            type="button"
            className="timeline-load-more"
            onClick={() => setVisibleCount((count) => count + 200)}
          >
            Show 200 more sessions
          </button>
        )}

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
      {editingId !== null && (
        <div className="timeline-dialog-backdrop" role="presentation">
          <section
            className="timeline-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="edit-session-title"
          >
            <div>
              <p className="section-kicker">Session</p>
              <h2 id="edit-session-title">Edit recorded time</h2>
              <span>Adjust the start and end of this closed session.</span>
            </div>
            <label>
              Start
              <input
                type="datetime-local"
                value={editStart}
                onChange={(event) => setEditStart(event.currentTarget.value)}
              />
            </label>
            <label>
              End
              <input
                type="datetime-local"
                value={editEnd}
                onChange={(event) => setEditEnd(event.currentTarget.value)}
              />
            </label>
            <div className="timeline-dialog-actions">
              <button type="button" onClick={() => setEditingId(null)}>Cancel</button>
              <button className="primary" type="button" onClick={() => {
                const startedAtMs = new Date(editStart).getTime();
                const endedAtMs = new Date(editEnd).getTime();
                if (!Number.isFinite(startedAtMs) || endedAtMs <= startedAtMs) {
                  setEditError("Session end must be after its start.");
                  return;
                }
                void updateTimelineSession(editingId, startedAtMs, endedAtMs)
                  .then(() => {
                    setEditingId(null);
                    setEditError(null);
                    notifyActivityDataChanged();
                    refresh();
                  })
                  .catch((reason) => setEditError(errorMessage(reason)));
              }}>Save changes</button>
            </div>
          </section>
        </div>
      )}
      {actionDialog && (
        <div className="timeline-dialog-backdrop" role="presentation">
          <section
            className="timeline-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="timeline-action-title"
          >
            <div>
              <p className="section-kicker">Timeline operation</p>
              <h2 id="timeline-action-title">
                {actionDialog.kind === "note"
                  ? "Update session notes"
                  : actionDialog.kind === "category"
                    ? "Change application category"
                    : "Delete recorded sessions"}
              </h2>
              <span>{actionDialog.label}</span>
            </div>
            {actionDialog.kind === "note" && (
              <label>
                Note
                <textarea
                  autoFocus
                  maxLength={500}
                  rows={4}
                  value={dialogValue}
                  placeholder="Leave empty to clear existing notes"
                  onChange={(event) => setDialogValue(event.currentTarget.value)}
                />
              </label>
            )}
            {actionDialog.kind === "category" && (
              <label>
                Category
                <input
                  autoFocus
                  maxLength={40}
                  value={dialogValue}
                  placeholder="Work, Communication, Learning…"
                  onChange={(event) => setDialogValue(event.currentTarget.value)}
                />
              </label>
            )}
            {actionDialog.kind === "delete" && (
              <p className="timeline-dialog-warning">
                These sessions will be removed from the timeline. You can undo this operation afterward.
              </p>
            )}
            <div className="timeline-dialog-actions">
              <button type="button" onClick={() => setActionDialog(null)}>Cancel</button>
              <button
                className={actionDialog.kind === "delete" ? "danger" : "primary"}
                type="button"
                onClick={submitActionDialog}
              >
                {actionDialog.kind === "delete" ? "Delete sessions" : "Apply changes"}
              </button>
            </div>
          </section>
        </div>
      )}
      {splittingId !== null && (
        <div className="timeline-dialog-backdrop" role="presentation">
          <section
            className="timeline-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="split-session-title"
          >
            <div>
              <p className="section-kicker">Session</p>
              <h2 id="split-session-title">Split recorded session</h2>
              <span>Create two adjacent sessions at the selected time.</span>
            </div>
            <label>
              Split at
              <input
                type="datetime-local"
                value={splitAt}
                onChange={(event) => setSplitAt(event.currentTarget.value)}
              />
            </label>
            <div className="timeline-dialog-actions">
              <button type="button" onClick={() => setSplittingId(null)}>Cancel</button>
              <button className="primary" type="button" onClick={() => {
                const splitAtMs = new Date(splitAt).getTime();
                if (!Number.isFinite(splitAtMs)) {
                  setEditError("Choose a valid split time.");
                  return;
                }
                void splitTimelineSession(splittingId, splitAtMs)
                  .then((result) => {
                    setSplittingId(null);
                    completeMutation("Split session into two parts.", result.undoToken);
                  })
                  .catch((reason) => setEditError(errorMessage(reason)));
              }}>Split session</button>
            </div>
          </section>
        </div>
      )}
      </>
      )}
    </div>
  );
}

function toLocalDateTimeInput(timestamp: number): string {
  const date = new Date(timestamp);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(timestamp - offset).toISOString().slice(0, 16);
}
