import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  TimelineEntry,
  TimelineFilters,
  errorMessage,
  getTimelinePage,
} from "../../lib/ipc";
import { ACTIVITY_DATA_CHANGED } from "../../lib/events";
import { appendTimelinePage } from "./timelinePageModel";

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
  loadAll: () => Promise<TimelineEntry[] | null>;
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
  const paginationRequest = useRef<object | null>(null);
  const nextOffset = useRef(0);
  const requestFilters = useMemo<TimelineFilters>(() => ({
    query: filters.query ?? null,
    state: filters.state ?? null,
    minimumDurationMs: filters.minimumDurationMs ?? null,
    maximumDurationMs: filters.maximumDurationMs ?? null,
    timeFromMinutes: filters.timeFromMinutes ?? null,
    timeToMinutes: filters.timeToMinutes ?? null,
    projectId: filters.projectId ?? null,
    tagId: filters.tagId ?? null,
    unassignedOnly: filters.unassignedOnly ?? false,
  }), [
    filters.maximumDurationMs,
    filters.minimumDurationMs,
    filters.query,
    filters.state,
    filters.timeFromMinutes,
    filters.timeToMinutes,
    filters.projectId,
    filters.tagId,
    filters.unassignedOnly,
  ]);

  const load = useCallback(async () => {
    const revision = ++requestRevision.current;
    paginationRequest.current = null;
    nextOffset.current = 0;
    setState((current) => ({ ...current, loading: true }));
    try {
      const page = await getTimelinePage(date, 0, 200, requestFilters);
      if (revision !== requestRevision.current) return;
      nextOffset.current = page.offset + page.entries.length;
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
    if (paginationRequest.current || state.loading || !state.hasMore) return;
    const revision = requestRevision.current;
    const offset = nextOffset.current;
    const request = {};
    paginationRequest.current = request;
    setState((current) => ({ ...current, loading: true }));
    try {
      const page = await getTimelinePage(date, offset, 200, requestFilters);
      if (revision !== requestRevision.current) return;
      if (page.offset === offset) nextOffset.current = page.offset + page.entries.length;
      setState((current) => {
        const entries = appendTimelinePage(current.entries, page, offset);
        return entries === null ? { ...current, loading: false } : {
          ...current,
          entries,
          loading: false,
          error: null,
          hasMore: page.hasMore,
        };
      });
    } catch (error) {
      if (revision !== requestRevision.current) return;
      setState((current) => ({ ...current, loading: false, error: errorMessage(error) }));
    } finally {
      if (paginationRequest.current === request) paginationRequest.current = null;
    }
  }, [date, requestFilters, state.hasMore, state.loading]);

  const loadAll = useCallback(async () => {
    if (paginationRequest.current || state.loading) return null;
    if (!state.hasMore) return state.entries;
    const revision = requestRevision.current;
    const request = {};
    paginationRequest.current = request;
    setState((current) => ({ ...current, loading: true }));
    try {
      let entries = state.entries;
      let offset = nextOffset.current;
      let hasMore: boolean = state.hasMore;
      while (hasMore) {
        const remaining = await getTimelinePage(date, offset, 1_000, requestFilters);
        if (revision !== requestRevision.current) return null;
        const merged = appendTimelinePage(entries, remaining, offset);
        if (merged === null || (remaining.entries.length === 0 && remaining.hasMore)) {
          setState((current) => ({ ...current, loading: false }));
          return null;
        }
        if (remaining.entries.length === 0) {
          hasMore = false;
          break;
        }
        entries = merged;
        offset = remaining.offset + remaining.entries.length;
        hasMore = remaining.hasMore;
      }
      nextOffset.current = offset;
      setState((current) => ({
        ...current,
        entries,
        loading: false,
        error: null,
        hasMore,
      }));
      return entries;
    } catch (error) {
      if (revision !== requestRevision.current) return null;
      setState((current) => ({ ...current, loading: false, error: errorMessage(error) }));
      return null;
    } finally {
      if (paginationRequest.current === request) paginationRequest.current = null;
    }
  }, [date, requestFilters, state.entries, state.hasMore, state.loading]);

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
    loadAll,
  };
}
