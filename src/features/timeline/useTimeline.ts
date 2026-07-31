import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  TimelineEntry,
  TimelineFilters,
  errorMessage,
  getTimelinePage,
} from "../../lib/ipc";
import { ACTIVITY_DATA_CHANGED } from "../../lib/events";

interface TimelineState {
  entries: TimelineEntry[];
  loading: boolean;
  error: string | null;
  totalCount: number;
  activeDurationMs: number;
  idleDurationMs: number;
  hasMore: boolean;
}

export function useTimeline(date: string, filters: TimelineFilters = {}): TimelineState & {
  refresh: () => void;
  loadMore: () => void;
  loadAll: () => void;
} {
  const [state, setState] = useState<TimelineState>({
    entries: [],
    loading: true,
    error: null,
    totalCount: 0,
    activeDurationMs: 0,
    idleDurationMs: 0,
    hasMore: false,
  });
  const requestRevision = useRef(0);
  const requestFilters = useMemo<TimelineFilters>(() => ({
    query: filters.query ?? null,
    state: filters.state ?? null,
    minimumDurationMs: filters.minimumDurationMs ?? null,
    maximumDurationMs: filters.maximumDurationMs ?? null,
    timeFromMinutes: filters.timeFromMinutes ?? null,
    timeToMinutes: filters.timeToMinutes ?? null,
  }), [
    filters.maximumDurationMs,
    filters.minimumDurationMs,
    filters.query,
    filters.state,
    filters.timeFromMinutes,
    filters.timeToMinutes,
  ]);

  const load = useCallback(async () => {
    const revision = ++requestRevision.current;
    setState((current) => ({ ...current, loading: true }));
    try {
      const page = await getTimelinePage(date, 0, 200, requestFilters);
      if (revision !== requestRevision.current) return;
      setState({
        entries: page.entries,
        loading: false,
        error: null,
        totalCount: page.totalCount,
        activeDurationMs: page.activeDurationMs,
        idleDurationMs: page.idleDurationMs,
        hasMore: page.hasMore,
      });
    } catch (error) {
      if (revision !== requestRevision.current) return;
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(error),
      }));
    }
  }, [date, requestFilters]);

  const loadMore = useCallback(async () => {
    if (state.loading || !state.hasMore) return;
    const revision = requestRevision.current;
    setState((current) => ({ ...current, loading: true }));
    try {
      const page = await getTimelinePage(date, state.entries.length, 200, requestFilters);
      if (revision !== requestRevision.current) return;
      setState((current) => ({
        ...current,
        entries: [...current.entries, ...page.entries],
        loading: false,
        error: null,
        hasMore: page.hasMore,
      }));
    } catch (error) {
      setState((current) => ({ ...current, loading: false, error: errorMessage(error) }));
    }
  }, [date, requestFilters, state.entries.length, state.hasMore, state.loading]);

  const loadAll = useCallback(async () => {
    if (state.loading || !state.hasMore) return;
    const revision = requestRevision.current;
    setState((current) => ({ ...current, loading: true }));
    try {
      const remaining = await getTimelinePage(date, state.entries.length, 1_000, requestFilters);
      if (revision !== requestRevision.current) return;
      setState((current) => ({
        ...current,
        entries: [...current.entries, ...remaining.entries],
        loading: false,
        error: null,
        hasMore: remaining.hasMore,
      }));
    } catch (error) {
      setState((current) => ({ ...current, loading: false, error: errorMessage(error) }));
    }
  }, [date, requestFilters, state.entries.length, state.hasMore, state.loading]);

  useEffect(() => {
    void load();
    window.addEventListener(ACTIVITY_DATA_CHANGED, load);
    return () => {
      window.removeEventListener(ACTIVITY_DATA_CHANGED, load);
    };
  }, [date, load]);

  return {
    ...state,
    refresh: () => void load(),
    loadMore: () => void loadMore(),
    loadAll: () => void loadAll(),
  };
}
