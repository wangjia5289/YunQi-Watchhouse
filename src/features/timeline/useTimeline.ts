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
    if (paginationRequest.current || state.loading || !state.hasMore) return;
    const revision = requestRevision.current;
    const offset = nextOffset.current;
    const request = {};
    paginationRequest.current = request;
    setState((current) => ({ ...current, loading: true }));
    try {
      const remaining = await getTimelinePage(date, offset, 1_000, requestFilters);
      if (revision !== requestRevision.current) return;
      if (remaining.offset === offset) {
        nextOffset.current = remaining.offset + remaining.entries.length;
      }
      setState((current) => {
        const entries = appendTimelinePage(current.entries, remaining, offset);
        return entries === null ? { ...current, loading: false } : {
          ...current,
          entries,
          loading: false,
          error: null,
          hasMore: remaining.hasMore,
        };
      });
    } catch (error) {
      if (revision !== requestRevision.current) return;
      setState((current) => ({ ...current, loading: false, error: errorMessage(error) }));
    } finally {
      if (paginationRequest.current === request) paginationRequest.current = null;
    }
  }, [date, requestFilters, state.hasMore, state.loading]);

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
