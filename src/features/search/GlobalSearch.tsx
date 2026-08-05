import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  dateFromLocalIso,
  formatClock,
  formatDuration,
  localIsoDate,
} from "../../lib/format";
import {
  ActivityState,
  ActivityTag,
  Project,
  TimelineEntry,
  TimelineFilters,
  errorMessage,
  listActivityTags,
  listProjects,
  searchTimelineRange,
} from "../../lib/ipc";
import { useLocale } from "../../lib/i18n";
import { ApplicationIcon } from "../applications/ApplicationIcon";
import {
  RangePreset,
  groupEntriesByDate,
  inclusiveDayCount,
  rangeForPreset,
} from "./searchModel";
import "./GlobalSearch.css";

const PAGE_SIZE = 200;
const SAVED_SEARCHES_KEY = "watchhouse.globalSearch.saved.v1";
const MAX_SAVED_SEARCHES = 12;

interface SavedSearch {
  id: string;
  name: string;
  preset: RangePreset;
  startDate: string;
  endDate: string;
  query: string;
  stateFilter: "ALL" | ActivityState;
  minimumMinutes: string;
  maximumMinutes: string;
  timeFrom: string;
  timeTo: string;
  projectId?: number | null;
  tagId?: number | null;
  unassignedOnly?: boolean;
}

interface SearchState {
  entries: TimelineEntry[];
  totalCount: number;
  activeDurationMs: number;
  idleDurationMs: number;
  hasMore: boolean;
  loading: boolean;
  error: string | null;
}

interface GlobalSearchProps {
  onOpenDate: (date: string, sessionId?: number) => void;
}

function optionalDurationMilliseconds(value: string): number | null {
  if (!value) return null;
  const minutes = Number(value);
  const milliseconds = minutes * 60_000;
  return Number.isFinite(milliseconds) && milliseconds >= 0 ? milliseconds : null;
}

function optionalClockMinutes(value: string): number | null {
  if (!value) return null;
  const [hours, minutes] = value.split(":").map(Number);
  return Number.isInteger(hours) && Number.isInteger(minutes)
    ? hours * 60 + minutes
    : null;
}

function readSavedSearches(): SavedSearch[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(SAVED_SEARCHES_KEY) ?? "[]");
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((item): item is SavedSearch => {
        if (!item || typeof item !== "object") return false;
        const candidate = item as Partial<SavedSearch>;
        return typeof candidate.id === "string"
          && typeof candidate.name === "string"
          && ["7_DAYS", "30_DAYS", "CUSTOM"].includes(candidate.preset ?? "")
          && typeof candidate.startDate === "string"
          && typeof candidate.endDate === "string"
          && typeof candidate.query === "string"
          && ["ALL", "ACTIVE", "IDLE"].includes(candidate.stateFilter ?? "")
          && typeof candidate.minimumMinutes === "string"
          && typeof candidate.maximumMinutes === "string"
          && typeof candidate.timeFrom === "string"
          && typeof candidate.timeTo === "string"
          && (candidate.projectId === undefined
            || candidate.projectId === null
            || (Number.isSafeInteger(candidate.projectId) && candidate.projectId > 0))
          && (candidate.tagId === undefined
            || candidate.tagId === null
            || (Number.isSafeInteger(candidate.tagId) && candidate.tagId > 0))
          && (candidate.unassignedOnly === undefined
            || typeof candidate.unassignedOnly === "boolean")
          && !(candidate.unassignedOnly
            && (candidate.projectId != null || candidate.tagId != null));
      })
      .slice(0, MAX_SAVED_SEARCHES);
  } catch {
    return [];
  }
}

function IdleBadge() {
  return (
    <span className="global-search-app-badge idle" aria-hidden="true">
      <svg viewBox="0 0 24 24">
        <path d="M7 15c1.2 2 3 3 5 3 3.3 0 6-2.7 6-6 0-2-.9-3.8-2.4-4.9.3.7.4 1.3.4 1.9a5 5 0 0 1-9 3z" />
      </svg>
    </span>
  );
}

export function GlobalSearch({ onOpenDate }: GlobalSearchProps) {
  const { locale, t } = useLocale();
  const today = localIsoDate();
  const initialRange = rangeForPreset("7_DAYS", today);
  const [preset, setPreset] = useState<RangePreset>("7_DAYS");
  const [startDate, setStartDate] = useState(initialRange.startDate);
  const [endDate, setEndDate] = useState(initialRange.endDate);
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [stateFilter, setStateFilter] = useState<"ALL" | ActivityState>("ALL");
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [minimumMinutes, setMinimumMinutes] = useState("");
  const [maximumMinutes, setMaximumMinutes] = useState("");
  const [timeFrom, setTimeFrom] = useState("");
  const [timeTo, setTimeTo] = useState("");
  const [projectId, setProjectId] = useState<number | null>(null);
  const [tagId, setTagId] = useState<number | null>(null);
  const [unassignedOnly, setUnassignedOnly] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [tags, setTags] = useState<ActivityTag[]>([]);
  const [savedSearchName, setSavedSearchName] = useState("");
  const [savedSearches, setSavedSearches] = useState<SavedSearch[]>(readSavedSearches);
  const [state, setState] = useState<SearchState>({
    entries: [],
    totalCount: 0,
    activeDurationMs: 0,
    idleDurationMs: 0,
    hasMore: false,
    loading: true,
    error: null,
  });
  const requestRevision = useRef(0);

  useEffect(() => {
    const timeout = window.setTimeout(() => setDebouncedQuery(query), 180);
    return () => window.clearTimeout(timeout);
  }, [query]);

  const minimumDurationMs = optionalDurationMilliseconds(minimumMinutes);
  const maximumDurationMs = optionalDurationMilliseconds(maximumMinutes);
  const timeFromMinutes = optionalClockMinutes(timeFrom);
  const timeToMinutes = optionalClockMinutes(timeTo);
  const filters = useMemo<TimelineFilters>(() => ({
    query: debouncedQuery.trim() || null,
    state: stateFilter === "ALL" ? null : stateFilter,
    minimumDurationMs,
    maximumDurationMs,
    timeFromMinutes,
    timeToMinutes,
    projectId,
    tagId,
    unassignedOnly,
  }), [
    debouncedQuery,
    maximumDurationMs,
    minimumDurationMs,
    stateFilter,
    timeFromMinutes,
    timeToMinutes,
    projectId,
    tagId,
    unassignedOnly,
  ]);

  const rangeIsValid = Boolean(startDate && endDate && startDate <= endDate);
  const dateValidationError = !rangeIsValid
    ? "End date must be on or after start date."
    : inclusiveDayCount(startDate, endDate) > 366
      ? "Date range cannot exceed 366 days."
      : null;
  const filterValidationError = minimumDurationMs !== null
      && maximumDurationMs !== null
      && minimumDurationMs > maximumDurationMs
    ? "Minimum duration cannot exceed maximum duration."
    : timeFromMinutes !== null
        && timeToMinutes !== null
        && timeFromMinutes > timeToMinutes
      ? "Start time cannot be after end time."
      : null;
  const validationError = dateValidationError ?? filterValidationError;
  const searchIsValid = validationError === null;

  const load = useCallback(async () => {
    const revision = ++requestRevision.current;
    if (!searchIsValid) {
      setState({
        entries: [],
        totalCount: 0,
        activeDurationMs: 0,
        idleDurationMs: 0,
        hasMore: false,
        loading: false,
        error: null,
      });
      return;
    }
    setState((current) => ({ ...current, loading: true, error: null }));
    try {
      const page = await searchTimelineRange(
        startDate,
        endDate,
        0,
        PAGE_SIZE,
        filters,
      );
      if (revision !== requestRevision.current) return;
      setState({
        entries: page.entries,
        totalCount: page.totalCount,
        activeDurationMs: page.activeDurationMs,
        idleDurationMs: page.idleDurationMs,
        hasMore: page.hasMore,
        loading: false,
        error: null,
      });
    } catch (reason) {
      if (revision !== requestRevision.current) return;
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(reason),
      }));
    }
  }, [endDate, filters, searchIsValid, startDate]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let active = true;
    void Promise.all([listProjects(true), listActivityTags(true)])
      .then(([nextProjects, nextTags]) => {
        if (!active) return;
        setProjects(nextProjects);
        setTags(nextTags);
      })
      .catch((reason) => {
        if (active) setState((current) => ({ ...current, error: errorMessage(reason) }));
      });
    return () => {
      active = false;
    };
  }, []);

  const loadMore = async () => {
    if (state.loading || !state.hasMore || !searchIsValid) return;
    const revision = requestRevision.current;
    setState((current) => ({ ...current, loading: true, error: null }));
    try {
      const page = await searchTimelineRange(
        startDate,
        endDate,
        state.entries.length,
        PAGE_SIZE,
        filters,
      );
      if (revision !== requestRevision.current) return;
      setState((current) => ({
        ...current,
        entries: [...current.entries, ...page.entries],
        hasMore: page.hasMore,
        loading: false,
        error: null,
      }));
    } catch (reason) {
      if (revision !== requestRevision.current) return;
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(reason),
      }));
    }
  };

  const choosePreset = (nextPreset: Exclude<RangePreset, "CUSTOM">) => {
    const nextRange = rangeForPreset(nextPreset, today);
    setPreset(nextPreset);
    setStartDate(nextRange.startDate);
    setEndDate(nextRange.endDate);
  };

  const clearFilters = () => {
    setQuery("");
    setStateFilter("ALL");
    setMinimumMinutes("");
    setMaximumMinutes("");
    setTimeFrom("");
    setTimeTo("");
    setProjectId(null);
    setTagId(null);
    setUnassignedOnly(false);
  };

  const persistSavedSearches = (next: SavedSearch[]) => {
    setSavedSearches(next);
    try {
      localStorage.setItem(SAVED_SEARCHES_KEY, JSON.stringify(next));
    } catch {
      // Search remains usable when WebView storage is unavailable.
    }
  };

  const saveCurrentSearch = () => {
    const name = savedSearchName.trim();
    if (!name || !searchIsValid) return;
    const existing = savedSearches.find(
      (saved) => saved.name.toLocaleLowerCase() === name.toLocaleLowerCase(),
    );
    const saved: SavedSearch = {
      id: existing?.id ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`,
      name,
      preset,
      startDate,
      endDate,
      query,
      stateFilter,
      minimumMinutes,
      maximumMinutes,
      timeFrom,
      timeTo,
      projectId,
      tagId,
      unassignedOnly,
    };
    persistSavedSearches([
      saved,
      ...savedSearches.filter((item) => item.id !== saved.id),
    ].slice(0, MAX_SAVED_SEARCHES));
    setSavedSearchName("");
  };

  const applySavedSearch = (saved: SavedSearch) => {
    setPreset(saved.preset);
    if (saved.preset === "CUSTOM") {
      setStartDate(saved.startDate);
      setEndDate(saved.endDate);
    } else {
      const range = rangeForPreset(saved.preset, today);
      setStartDate(range.startDate);
      setEndDate(range.endDate);
    }
    setQuery(saved.query);
    setStateFilter(saved.stateFilter);
    setMinimumMinutes(saved.minimumMinutes);
    setMaximumMinutes(saved.maximumMinutes);
    setTimeFrom(saved.timeFrom);
    setTimeTo(saved.timeTo);
    setProjectId(saved.projectId ?? null);
    setTagId(saved.tagId ?? null);
    setUnassignedOnly(saved.unassignedOnly ?? false);
    setAdvancedOpen(Boolean(
      saved.minimumMinutes
        || saved.maximumMinutes
        || saved.timeFrom
        || saved.timeTo
        || saved.projectId
        || saved.tagId
        || saved.unassignedOnly,
    ));
  };

  const hasFilters = Boolean(
    query.trim()
      || stateFilter !== "ALL"
      || minimumMinutes
      || maximumMinutes
      || timeFrom
      || timeTo
      || projectId !== null
      || tagId !== null
      || unassignedOnly,
  );
  const groups = useMemo(() => groupEntriesByDate(state.entries), [state.entries]);
  const dateFormatter = useMemo(() => new Intl.DateTimeFormat(locale, {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  }), [locale]);

  return (
    <div className="global-search-page">
      <header className="global-search-header">
        <div>
          <p>{t("Explore activity")}</p>
          <h1>{t("Search")}</h1>
        </div>
        <div className="global-search-range-tabs" role="group" aria-label={t("Search range")}>
          <button
            type="button"
            className={preset === "7_DAYS" ? "active" : ""}
            aria-pressed={preset === "7_DAYS"}
            onClick={() => choosePreset("7_DAYS")}
          >
            {t("Last 7 days")}
          </button>
          <button
            type="button"
            className={preset === "30_DAYS" ? "active" : ""}
            aria-pressed={preset === "30_DAYS"}
            onClick={() => choosePreset("30_DAYS")}
          >
            {t("Last 30 days")}
          </button>
          <button
            type="button"
            className={preset === "CUSTOM" ? "active" : ""}
            aria-pressed={preset === "CUSTOM"}
            onClick={() => setPreset("CUSTOM")}
          >
            {t("Custom range")}
          </button>
        </div>
      </header>

      {preset === "CUSTOM" && (
        <div className="global-search-date-range">
          <label>
            <span>{t("Start date")}</span>
            <input
              type="date"
              value={startDate}
              max={endDate || today}
              onChange={(event) => setStartDate(event.currentTarget.value)}
            />
          </label>
          <label>
            <span>{t("End date")}</span>
            <input
              type="date"
              value={endDate}
              min={startDate}
              max={today}
              onChange={(event) => setEndDate(event.currentTarget.value)}
            />
          </label>
          {dateValidationError && (
            <span className="global-search-date-error" role="alert">
              {t(dateValidationError)}
            </span>
          )}
        </div>
      )}

      <section className="global-search-saved" aria-label={t("Saved searches")}>
        <strong>{t("Saved searches")}</strong>
        <div className="global-search-saved-list">
          {savedSearches.map((saved) => (
            <span className="global-search-saved-item" key={saved.id}>
              <button type="button" onClick={() => applySavedSearch(saved)}>
                {saved.name}
              </button>
              <button
                type="button"
                className="global-search-remove-saved"
                aria-label={`${t("Remove saved search")}: ${saved.name}`}
                title={t("Remove saved search")}
                onClick={() => persistSavedSearches(
                  savedSearches.filter((item) => item.id !== saved.id),
                )}
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="m7 7 10 10M17 7 7 17" />
                </svg>
              </button>
            </span>
          ))}
        </div>
        <form
          className="global-search-save-form"
          onSubmit={(event) => {
            event.preventDefault();
            saveCurrentSearch();
          }}
        >
          <input
            type="text"
            value={savedSearchName}
            maxLength={40}
            aria-label={t("Search name")}
            placeholder={t("Search name")}
            onChange={(event) => setSavedSearchName(event.currentTarget.value)}
          />
          <button type="submit" disabled={!savedSearchName.trim() || !searchIsValid}>
            {t("Save search")}
          </button>
        </form>
      </section>

      <section className="global-search-summary" aria-label={t("Search result summary")}>
        <div>
          <span className="global-search-summary-dot active" />
          <p>{t("Active")}</p>
          <strong>{formatDuration(state.activeDurationMs, locale)}</strong>
        </div>
        <div>
          <span className="global-search-summary-dot idle" />
          <p>{t("Idle")}</p>
          <strong>{formatDuration(state.idleDurationMs, locale)}</strong>
        </div>
        <div>
          <span className="global-search-summary-dot sessions" />
          <p>{t("Matching sessions")}</p>
          <strong>{state.totalCount}</strong>
        </div>
      </section>

      <section className="global-search-controls" aria-label={t("Search filters")}>
        <div className="global-search-main-filters">
          <label className="global-search-query">
            <span>{t("Search")}</span>
            <input
              type="search"
              value={query}
              maxLength={200}
              placeholder={t("App, title, note, project, tag, bundle, or category")}
              onChange={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
          <label>
            <span>{t("State")}</span>
            <select
              value={stateFilter}
              onChange={(event) => setStateFilter(
                event.currentTarget.value as "ALL" | ActivityState,
              )}
            >
              <option value="ALL">{t("All activity")}</option>
              <option value="ACTIVE">{t("Active")}</option>
              <option value="IDLE">{t("Idle")}</option>
            </select>
          </label>
          <button
            type="button"
            aria-expanded={advancedOpen}
            onClick={() => setAdvancedOpen((open) => !open)}
          >
            {t(advancedOpen ? "Hide advanced" : "Advanced")}
          </button>
          {hasFilters && (
            <button type="button" onClick={clearFilters}>
              {t("Clear")}
            </button>
          )}
        </div>
        {advancedOpen && (
          <div className="global-search-advanced">
            <label>
              <span>{t("Minimum minutes")}</span>
              <input
                type="number"
                min="0"
                value={minimumMinutes}
                onChange={(event) => setMinimumMinutes(event.currentTarget.value)}
              />
            </label>
            <label>
              <span>{t("Maximum minutes")}</span>
              <input
                type="number"
                min="0"
                value={maximumMinutes}
                onChange={(event) => setMaximumMinutes(event.currentTarget.value)}
              />
            </label>
            <label>
              <span>{t("From")}</span>
              <input
                type="time"
                value={timeFrom}
                onChange={(event) => setTimeFrom(event.currentTarget.value)}
              />
            </label>
            <label>
              <span>{t("To")}</span>
              <input
                type="time"
                value={timeTo}
                onChange={(event) => setTimeTo(event.currentTarget.value)}
              />
            </label>
            <label>
              <span>{t("Project")}</span>
              <select
                value={projectId ?? ""}
                disabled={unassignedOnly}
                onChange={(event) => setProjectId(
                  event.currentTarget.value ? Number(event.currentTarget.value) : null,
                )}
              >
                <option value="">{t("All projects")}</option>
                {projects.map((project) => (
                  <option value={project.id} key={project.id}>{project.name}</option>
                ))}
              </select>
            </label>
            <label>
              <span>{t("Activity tag")}</span>
              <select
                value={tagId ?? ""}
                disabled={unassignedOnly}
                onChange={(event) => setTagId(
                  event.currentTarget.value ? Number(event.currentTarget.value) : null,
                )}
              >
                <option value="">{t("All tags")}</option>
                {tags.map((tag) => (
                  <option value={tag.id} key={tag.id}>{tag.name}</option>
                ))}
              </select>
            </label>
            <label className="organization-unassigned-filter">
              <input
                type="checkbox"
                checked={unassignedOnly}
                onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  setUnassignedOnly(checked);
                  if (checked) {
                    setProjectId(null);
                    setTagId(null);
                  }
                }}
              />
              <span>{t("Unassigned only")}</span>
            </label>
          </div>
        )}
      </section>

      {filterValidationError && (
        <div className="global-search-error" role="alert">
          <span>{t(filterValidationError)}</span>
        </div>
      )}

      {state.error && (
        <div className="global-search-error" role="alert">
          <span>{t(state.error)}</span>
          <button type="button" onClick={() => void load()}>
            {t("Try again")}
          </button>
        </div>
      )}

      <section className="global-search-results" aria-label={t("Search results")}>
        {state.loading && state.entries.length === 0 && (
          <div
            className="global-search-loading"
            role="status"
            aria-label={t("Loading search results")}
          >
            <span />
            <span />
            <span />
          </div>
        )}

        {!state.loading && !state.error && state.entries.length === 0 && searchIsValid && (
          <div className="global-search-empty">
            <span aria-hidden="true">
              <svg viewBox="0 0 24 24">
                <circle cx="11" cy="11" r="6" />
                <path d="m16 16 4 4" />
              </svg>
            </span>
            <h2>{t("No matching activity")}</h2>
            <p>{t("Try another date range or adjust the filters.")}</p>
          </div>
        )}

        {groups.map(([date, entries]) => (
          <section className="global-search-day" key={date}>
            <header>
              <div>
                <h2>{dateFormatter.format(dateFromLocalIso(date))}</h2>
                <span>{t(`${entries.length} shown on this date`)}</span>
              </div>
              <button type="button" onClick={() => onOpenDate(date)}>
                {t("Open in Timeline")}
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="m9 5 7 7-7 7" />
                </svg>
              </button>
            </header>
            <div className="global-search-day-entries">
              {entries.map((entry) => {
                const idle = entry.state === "IDLE";
                const name = idle
                  ? t("Idle")
                  : entry.applicationName ?? t("Unknown application");
                return (
                  <button
                    type="button"
                    className={`global-search-entry${idle ? " idle" : ""}`}
                    key={entry.sessionId}
                    title={t("Open in Timeline")}
                    onClick={() => onOpenDate(date, entry.sessionId)}
                  >
                    {idle || entry.applicationId === null ? (
                      <IdleBadge />
                    ) : (
                      <ApplicationIcon
                        className="global-search-app-badge"
                        applicationId={entry.applicationId}
                        applicationName={name}
                      />
                    )}
                    <div className="global-search-entry-copy">
                      <strong>{name}</strong>
                      <span>
                        {formatClock(entry.startedAtMs, locale)}
                        {" - "}
                        {formatClock(entry.endedAtMs, locale)}
                        {entry.category && (
                          <i>{entry.category}</i>
                        )}
                      </span>
                      {entry.windowTitle && <small>{entry.windowTitle}</small>}
                      {entry.note && <small>{entry.note}</small>}
                      {(entry.project || entry.tags.length > 0) && (
                        <div className="session-organization-badges" aria-label={t("Organization")}>
                          {entry.project && (
                            <span className="session-project-badge">
                              <i style={{ backgroundColor: entry.project.color }} aria-hidden="true" />
                              {entry.project.name}
                            </span>
                          )}
                          {entry.tags.map((tag) => (
                            <span className="session-tag-badge" key={tag.id}>
                              <i style={{ backgroundColor: tag.color }} aria-hidden="true" />
                              {tag.name}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                    <span className="global-search-duration">
                      {formatDuration(entry.durationMs, locale)}
                    </span>
                  </button>
                );
              })}
            </div>
          </section>
        ))}

        {state.hasMore && (
          <button
            type="button"
            className="global-search-load-more"
            disabled={state.loading}
            onClick={() => void loadMore()}
          >
            {t(state.loading
              ? "Loading…"
              : `Show more results (${state.entries.length} of ${state.totalCount})`)}
          </button>
        )}
      </section>
    </div>
  );
}
