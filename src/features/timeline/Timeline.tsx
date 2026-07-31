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
  TimelineFilters,
  deleteTimelineSession,
  deleteTimelineSessions,
  errorMessage,
  getTimelineUndoHistory,
  importActivity,
  ImportPreview,
  TimelineUndoEntry,
  mergeTimelineSessions,
  previewActivityImport,
  splitTimelineSession,
  undoTimelineEdit,
  updateTimelineSessionCategories,
  updateTimelineSessionNotes,
  updateTimelineSession,
} from "../../lib/ipc";
import { notifyActivityDataChanged } from "../../lib/events";
import { useLocale } from "../../lib/i18n";
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
  const { t } = useLocale();
  return (
    <button
      className="day-arrow"
      type="button"
      aria-label={t(`${direction} day`)}
      disabled={disabled}
      onClick={onClick}
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d={direction === "previous" ? "m15 5-7 7 7 7" : "m9 5 7 7-7 7"} />
      </svg>
    </button>
  );
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

function optionalDurationMilliseconds(value: string): number | null {
  if (!value) return null;
  const minutes = Number(value);
  return Number.isFinite(minutes) && minutes >= 0 ? minutes * 60_000 : null;
}

function optionalClockMinutes(value: string): number | null {
  if (!value) return null;
  const [hours, minutes] = value.split(":").map(Number);
  return Number.isInteger(hours) && Number.isInteger(minutes)
    ? hours * 60 + minutes
    : null;
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
  const { locale, t } = useLocale();
  const today = localIsoDate();
  const [date, setDate] = useState(today);
  const [view, setView] = useState<"overview" | "details">("overview");
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [stateFilter, setStateFilter] = useState<"ALL" | ActivityState>("ALL");
  const [advancedSearchOpen, setAdvancedSearchOpen] = useState(false);
  const [minimumMinutes, setMinimumMinutes] = useState("");
  const [maximumMinutes, setMaximumMinutes] = useState("");
  const [timeFrom, setTimeFrom] = useState("");
  const [timeTo, setTimeTo] = useState("");
  const timelineFilters = useMemo<TimelineFilters>(() => ({
    query: debouncedQuery.trim() || null,
    state: stateFilter === "ALL" ? null : stateFilter,
    minimumDurationMs: optionalDurationMilliseconds(minimumMinutes),
    maximumDurationMs: optionalDurationMilliseconds(maximumMinutes),
    timeFromMinutes: optionalClockMinutes(timeFrom),
    timeToMinutes: optionalClockMinutes(timeTo),
  }), [
    debouncedQuery,
    maximumMinutes,
    minimumMinutes,
    stateFilter,
    timeFrom,
    timeTo,
  ]);
  const {
    entries,
    loading,
    error,
    refresh,
    loadMore,
    loadAll,
    hasMore,
    totalCount,
    activeDurationMs,
    idleDurationMs,
  } = useTimeline(date, timelineFilters);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [splittingId, setSplittingId] = useState<number | null>(null);
  const [splitAt, setSplitAt] = useState("");
  const [editStart, setEditStart] = useState("");
  const [editEnd, setEditEnd] = useState("");
  const [editError, setEditError] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [undoHistory, setUndoHistory] = useState<TimelineUndoEntry[]>([]);
  const [undoHistoryOpen, setUndoHistoryOpen] = useState(false);
  const [operationMessage, setOperationMessage] = useState<string | null>(null);
  const [actionDialog, setActionDialog] = useState<TimelineActionDialog | null>(null);
  const [dialogValue, setDialogValue] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [importContents, setImportContents] = useState("");
  const [importFormat, setImportFormat] = useState<"json" | "csv">("json");
  const [importPreview, setImportPreview] = useState<ImportPreview | null>(null);
  const [conflictPolicy, setConflictPolicy] = useState<"skip" | "merge">("skip");
  const filteredEntries = entries;
  const hours = useMemo(() => summarizeByHour(filteredEntries), [filteredEntries]);
  const visibleEntries = filteredEntries;

  useEffect(() => {
    const timeout = window.setTimeout(() => setDebouncedQuery(query), 180);
    return () => window.clearTimeout(timeout);
  }, [query]);
  useEffect(() => {
    setSelectedIds(new Set());
  }, [date, maximumMinutes, minimumMinutes, query, stateFilter, timeFrom, timeTo]);
  useEffect(() => {
    if (view === "overview" && hasMore && !loading) loadAll();
  }, [hasMore, loadAll, loading, view]);
  useEffect(() => {
    void getTimelineUndoHistory().then(setUndoHistory).catch(() => {});
  }, []);
  const selected = [...selectedIds];

  const completeMutation = (message: string, token?: string | null) => {
    setSelectedIds(new Set());
    if (token) {
      void getTimelineUndoHistory().then(setUndoHistory).catch(() => {});
    }
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
  const hasFilters = Boolean(
    query.trim() || stateFilter !== "ALL" || minimumMinutes || maximumMinutes || timeFrom || timeTo,
  );
  const activeTotal = activeDurationMs;
  const idleTotal = idleDurationMs;
  const dateTitle = isToday
    ? t("Today")
    : new Intl.DateTimeFormat(locale, {
        weekday: "long",
        month: "long",
        day: "numeric",
      }).format(dateValue);

  return (
    <div className="timeline-page">
      <header className="timeline-header">
        <div>
          <p className="date-label">{t("Computer activity")}</p>
          <h1>{t("Timeline")}</h1>
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
              aria-label={t("Timeline date")}
            />
          </label>
          <DayButton
            direction="next"
            disabled={isToday}
            onClick={() => setDate(shiftLocalDate(date, 1))}
          />
          {!isToday && (
            <button className="today-button" type="button" onClick={() => setDate(today)}>
              {t("Today")}
            </button>
          )}
        </div>
      </header>

      <section className="timeline-summary" aria-label={t("Selected day summary")}>
        <div>
          <span className="summary-swatch active" />
          <p>{t("Active")}</p>
          <strong>{formatDuration(activeTotal, locale)}</strong>
        </div>
        <div>
          <span className="summary-swatch idle" />
          <p>{t("Idle")}</p>
          <strong>{formatDuration(idleTotal, locale)}</strong>
        </div>
        <div>
          <span className="summary-swatch sessions" />
          <p>{t("Sessions")}</p>
          <strong>{totalCount}</strong>
        </div>
      </section>

      <div className="timeline-view-bar">
        <div>
          <p className="section-kicker">{t("Day structure")}</p>
          <strong>{view === "overview"
            ? t(`${hours.length} active time blocks`)
            : t(`${totalCount} matching sessions`)}</strong>
        </div>
        <div className="range-tabs" role="group" aria-label={t("Timeline view")}>
          <button className={view === "overview" ? "active" : ""} onClick={() => setView("overview")}>
            {t("Overview")}
          </button>
          <button className={view === "details" ? "active" : ""} onClick={() => setView("details")}>
            {t("Details")}
          </button>
        </div>
        <button className="timeline-import-button" type="button" onClick={() => setImportOpen((open) => !open)}>
          {t("Import")}
        </button>
        <button
          className="timeline-import-button"
          type="button"
          aria-expanded={undoHistoryOpen}
          onClick={() => setUndoHistoryOpen((open) => !open)}
        >
          {t("Undo history")}{undoHistory.length > 0 ? ` (${undoHistory.length})` : ""}
        </button>
      </div>

      {undoHistoryOpen && (
        <section className="timeline-undo-history" aria-label={t("Undo history")}>
          <div className="timeline-undo-heading">
            <div>
              <strong>{t("Recent timeline edits")}</strong>
              <span>{t("Undo snapshots expire after 24 hours.")}</span>
            </div>
            <button type="button" aria-label={t("Close undo history")} onClick={() => setUndoHistoryOpen(false)}>
              {t("Close")}
            </button>
          </div>
          {undoHistory.length === 0 ? (
            <p className="timeline-undo-empty">{t("No timeline edits can be undone.")}</p>
          ) : (
            <div className="timeline-undo-list">
              {[...undoHistory].reverse().map((entry) => {
                const expiresAt = entry.createdAtMs + 24 * 60 * 60 * 1_000;
                const hoursRemaining = Math.max(1, Math.ceil((expiresAt - Date.now()) / (60 * 60 * 1_000)));
                return (
                  <div key={entry.token}>
                    <span>
                      <strong>{t(entry.operationLabel)}</strong>
                      <small>
                        {t(`${entry.sessionCount} ${entry.sessionCount === 1 ? "session" : "sessions"}`)}
                        {" · "}{new Date(entry.createdAtMs).toLocaleString(locale)}
                        {" · "}{t(`${hoursRemaining}h remaining`)}
                      </small>
                    </span>
                    <button type="button" onClick={() => void runOperation(async () => {
                      const restored = await undoTimelineEdit(entry.token);
                      setUndoHistory((current) => current.filter((item) => item.token !== entry.token));
                      completeMutation(`Restored ${restored} sessions.`);
                    })}>{t("Undo")}</button>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      )}

      {importOpen && (
        <section className="timeline-import" aria-label={t("Import activity")}>
          <div>
            <strong>{t("Import Watchhouse data")}</strong>
            <span>{t("Choose a JSON or CSV export. Nothing is written until you confirm.")}</span>
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
              <span>{t(`${importPreview.recordCount} valid sessions`)}</span>
              <span>{t(`${importPreview.conflictCount} conflicts`)}</span>
              <span>{t(`${importPreview.invalidCount} invalid`)}</span>
              {importPreview.startedAtMs !== null && importPreview.endedAtMs !== null && (
                <span>{new Date(importPreview.startedAtMs).toLocaleDateString(locale)} – {new Date(importPreview.endedAtMs).toLocaleDateString(locale)}</span>
              )}
              <select value={conflictPolicy} onChange={(event) => setConflictPolicy(event.currentTarget.value as "skip" | "merge")} aria-label={t("Import conflict policy")}>
                <option value="skip">{t("Skip conflicts")}</option>
                <option value="merge">{t("Merge compatible conflicts")}</option>
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
              >{t("Import activity")}</button>
            </div>
          )}
        </section>
      )}

      <div className="timeline-filters">
        <label>
          <span>{t("Search")}</span>
          <input
            type="search"
            value={query}
            maxLength={200}
            placeholder={t("App, title, note, bundle, or category")}
            onChange={(event) => setQuery(event.currentTarget.value)}
          />
        </label>
        <label>
          <span>{t("State")}</span>
          <select
            value={stateFilter}
            onChange={(event) => setStateFilter(event.currentTarget.value as "ALL" | ActivityState)}
          >
            <option value="ALL">{t("All activity")}</option>
            <option value="ACTIVE">{t("Active")}</option>
            <option value="IDLE">{t("Idle")}</option>
          </select>
        </label>
        <button
          type="button"
          aria-expanded={advancedSearchOpen}
          onClick={() => setAdvancedSearchOpen((open) => !open)}
        >
          {t(advancedSearchOpen ? "Hide advanced" : "Advanced")}
        </button>
        {hasFilters && (
          <button type="button" onClick={() => {
            setQuery("");
            setStateFilter("ALL");
            setMinimumMinutes("");
            setMaximumMinutes("");
            setTimeFrom("");
            setTimeTo("");
          }}>{t("Clear")}</button>
        )}
      </div>
      {advancedSearchOpen && (
        <div className="timeline-advanced-filters">
          <label>
            <span>{t("Minimum minutes")}</span>
            <input type="number" min="0" value={minimumMinutes} onChange={(event) => setMinimumMinutes(event.currentTarget.value)} />
          </label>
          <label>
            <span>{t("Maximum minutes")}</span>
            <input type="number" min="0" value={maximumMinutes} onChange={(event) => setMaximumMinutes(event.currentTarget.value)} />
          </label>
          <label>
            <span>{t("From")}</span>
            <input type="time" value={timeFrom} onChange={(event) => setTimeFrom(event.currentTarget.value)} />
          </label>
          <label>
            <span>{t("To")}</span>
            <input type="time" value={timeTo} onChange={(event) => setTimeTo(event.currentTarget.value)} />
          </label>
        </div>
      )}
      {editError && <div className="error-banner" role="alert">{t(editError)}</div>}
      {operationMessage && (
        <div className="timeline-operation-message" role="status">
          <span>{t(operationMessage)}</span>
          {undoHistory.length > 0 && <button type="button" onClick={() => void runOperation(async () => {
            const entry = undoHistory[undoHistory.length - 1];
            const restored = await undoTimelineEdit(entry.token);
            setUndoHistory((current) => current.slice(0, -1));
            completeMutation(`Restored ${restored} sessions. ${undoHistory.length - 1} undo steps remain.`);
          })}>{t(`Undo (${undoHistory.length})`)}</button>}
          <button type="button" aria-label={t("Dismiss message")} onClick={() => {
            setOperationMessage(null);
          }}>{t("Close")}</button>
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

      {view === "overview" ? (
        <section className="timeline-overview" aria-label={`${dateTitle} ${t("Hourly overview")}`}>
          {loading && entries.length === 0 && (
            <div className="timeline-loading">
              <div className="skeleton timeline-skeleton" />
              <div className="skeleton timeline-skeleton short" />
            </div>
          )}
          {!loading && hours.length === 0 && !error && (
            <div className="empty-timeline">
              <h2>{t(hasFilters ? "No matching activity" : "No activity recorded")}</h2>
              <p>{t(hasFilters
                ? "Adjust or clear the filters to see other sessions."
                : "Watchhouse did not record any computer activity on this day.")}</p>
            </div>
          )}
          {hours.map((hour) => {
            const recorded = hour.activeDurationMs + hour.idleDurationMs;
            const activeShare = recorded ? hour.activeDurationMs / recorded * 100 : 0;
            return (
              <article className="hour-block" key={hour.startedAtMs}>
                <time>{formatClock(hour.startedAtMs, locale)}</time>
                <div className="hour-content">
                  <div className="hour-heading">
                    <div>
                      <strong>{t(`${formatDuration(hour.activeDurationMs, locale)} active`)}</strong>
                      <span>{t(`${formatDuration(hour.idleDurationMs, locale)} idle · ${hour.sessionCount} sessions`)}</span>
                    </div>
                    <span>{t(`${Math.round(activeShare)}% active`)}</span>
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
                        <span>{application.name === "Unknown application"
                          ? t(application.name)
                          : application.name}</span>
                        <strong>{formatDuration(application.durationMs, locale)}</strong>
                      </div>
                    )) : <small>{t("No active application in this hour.")}</small>}
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
            {t(selected.length ? `${selected.length} selected` : "Select sessions")}
          </label>
          {selected.length > 0 && (
            <div>
              <button type="button" disabled={selected.length < 2} onClick={() => void runOperation(async () => {
                const result = await mergeTimelineSessions(selected);
                completeMutation(`Merged ${result.affectedCount} sessions.`, result.undoToken);
              })}>{t("Merge")}</button>
              <button type="button" onClick={() => {
                setDialogValue("");
                setActionDialog({
                  kind: "note",
                  sessionIds: selected,
                  label: `${selected.length} selected sessions`,
                });
              }}>{t("Note")}</button>
              <button type="button" onClick={() => {
                setDialogValue("");
                setActionDialog({
                  kind: "category",
                  sessionIds: selected,
                  label: `${selected.length} selected sessions`,
                });
              }}>{t("Category")}</button>
              <button className="danger" type="button" onClick={() => {
                setActionDialog({
                  kind: "delete",
                  sessionIds: selected,
                  label: `${selected.length} selected sessions`,
                });
              }}>{t("Delete")}</button>
            </div>
          )}
        </div>
      )}
      <section className="timeline-list" aria-label={`${dateTitle} ${t("Activity sessions")}`}>
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
            <h2>{t(hasFilters ? "No matching activity" : "No activity recorded")}</h2>
            <p>{t(hasFilters
              ? "Adjust or clear the filters to see other sessions."
              : "Watchhouse did not record any computer activity on this day.")}</p>
          </div>
        )}

        {visibleEntries.map((entry, index) => {
          const idle = entry.state === "IDLE";
          const name = idle
            ? t("Idle")
            : entry.applicationName ?? t("Unknown application");
          return (
            <article className={`timeline-row${idle ? " idle" : ""}`} key={entry.sessionId}>
              {!entry.isOpen && (
                <input
                  className="session-select"
                  type="checkbox"
                  checked={selectedIds.has(entry.sessionId)}
                  aria-label={t(`Select ${name} session`)}
                  onChange={(event) => setSelectedIds((current) => {
                    const next = new Set(current);
                    if (event.currentTarget.checked) next.add(entry.sessionId);
                    else next.delete(entry.sessionId);
                    return next;
                  })}
                />
              )}
              <time>{formatClock(entry.startedAtMs, locale)}</time>
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
                    {formatClock(entry.startedAtMs, locale)} – {formatClock(entry.endedAtMs, locale)}
                    {entry.isOpen && <i className="open-session">{t("Live")}</i>}
                  </span>
                  {entry.windowTitle && (
                    <small className="session-note">{entry.windowTitle}</small>
                  )}
                  {entry.note && <small className="session-note">{entry.note}</small>}
                </div>
                <span className="session-duration">{formatDuration(entry.durationMs, locale)}</span>
                {!entry.isOpen && (
                  <>
                  <button
                    type="button"
                    className="edit-session"
                    aria-label={t(`Edit ${name} session`)}
                    title={t("Edit session time")}
                    onClick={() => {
                      setEditingId(entry.sessionId);
                      setEditStart(toLocalDateTimeInput(entry.startedAtMs));
                      setEditEnd(toLocalDateTimeInput(entry.endedAtMs));
                      setEditError(null);
                    }}
                  >{t("Edit")}</button>
                  <button
                    type="button"
                    className="edit-session"
                    aria-label={t(`Split ${name} session`)}
                    title={t("Split session")}
                    onClick={() => {
                      setSplittingId(entry.sessionId);
                      setSplitAt(toLocalDateTimeInput(
                        entry.startedAtMs + Math.floor(entry.durationMs / 2),
                      ));
                      setEditError(null);
                    }}
                  >{t("Split")}</button>
                  <button
                    type="button"
                    className="delete-session"
                    aria-label={t(`Delete ${name} session at ${formatClock(entry.startedAtMs, locale)}`)}
                    title={t("Delete session")}
                    onClick={() => {
                      setActionDialog({
                        kind: "delete",
                        sessionIds: [entry.sessionId],
                        label: `${name}, ${formatClock(entry.startedAtMs, locale)}–${formatClock(entry.endedAtMs, locale)}`,
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

        {hasMore && (
          <button
            type="button"
            className="timeline-load-more"
            disabled={loading}
            onClick={loadMore}
          >
            {t(loading ? "Loading…" : `Show more sessions (${entries.length} of ${totalCount})`)}
          </button>
        )}

        {filteredEntries.length > 0 && (
          <div className="timeline-end">
            <time>
              {formatClock(filteredEntries[filteredEntries.length - 1]?.endedAtMs ?? null, locale)}
            </time>
            <span />
            <p>{t("End of recorded activity")}</p>
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
              <p className="section-kicker">{t("Session")}</p>
              <h2 id="edit-session-title">{t("Edit recorded time")}</h2>
              <span>{t("Adjust the start and end of this closed session.")}</span>
            </div>
            <label>
              {t("Start")}
              <input
                type="datetime-local"
                value={editStart}
                onChange={(event) => setEditStart(event.currentTarget.value)}
              />
            </label>
            <label>
              {t("End")}
              <input
                type="datetime-local"
                value={editEnd}
                onChange={(event) => setEditEnd(event.currentTarget.value)}
              />
            </label>
            <div className="timeline-dialog-actions">
              <button type="button" onClick={() => setEditingId(null)}>{t("Cancel")}</button>
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
              }}>{t("Save changes")}</button>
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
              <p className="section-kicker">{t("Timeline operation")}</p>
              <h2 id="timeline-action-title">
                {t(actionDialog.kind === "note"
                  ? "Update session notes"
                  : actionDialog.kind === "category"
                    ? "Change application category"
                    : "Delete recorded sessions")}
              </h2>
              <span>{t(actionDialog.label)}</span>
            </div>
            {actionDialog.kind === "note" && (
              <label>
                {t("Note")}
                <textarea
                  autoFocus
                  maxLength={500}
                  rows={4}
                  value={dialogValue}
                  placeholder={t("Leave empty to clear existing notes")}
                  onChange={(event) => setDialogValue(event.currentTarget.value)}
                />
              </label>
            )}
            {actionDialog.kind === "category" && (
              <label>
                {t("Category")}
                <input
                  autoFocus
                  maxLength={40}
                  value={dialogValue}
                  placeholder={t("Work, Communication, Learning…")}
                  onChange={(event) => setDialogValue(event.currentTarget.value)}
                />
              </label>
            )}
            {actionDialog.kind === "delete" && (
              <p className="timeline-dialog-warning">
                {t("These sessions will be removed from the timeline. You can undo this operation afterward.")}
              </p>
            )}
            <div className="timeline-dialog-actions">
              <button type="button" onClick={() => setActionDialog(null)}>{t("Cancel")}</button>
              <button
                className={actionDialog.kind === "delete" ? "danger" : "primary"}
                type="button"
                onClick={submitActionDialog}
              >
                {t(actionDialog.kind === "delete" ? "Delete sessions" : "Apply changes")}
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
              <p className="section-kicker">{t("Session")}</p>
              <h2 id="split-session-title">{t("Split recorded session")}</h2>
              <span>{t("Create two adjacent sessions at the selected time.")}</span>
            </div>
            <label>
              {t("Split at")}
              <input
                type="datetime-local"
                value={splitAt}
                onChange={(event) => setSplitAt(event.currentTarget.value)}
              />
            </label>
            <div className="timeline-dialog-actions">
              <button type="button" onClick={() => setSplittingId(null)}>{t("Cancel")}</button>
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
              }}>{t("Split session")}</button>
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
