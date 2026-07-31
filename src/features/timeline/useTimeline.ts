import { useCallback, useEffect, useRef, useState } from "react";
import { TimelineEntry, errorMessage, getTimeline } from "../../lib/ipc";
import { ACTIVITY_DATA_CHANGED } from "../../lib/events";

interface TimelineState {
  entries: TimelineEntry[];
  loading: boolean;
  error: string | null;
}

export function useTimeline(date: string): TimelineState & { refresh: () => void } {
  const [state, setState] = useState<TimelineState>({
    entries: [],
    loading: true,
    error: null,
  });
  const requestRevision = useRef(0);

  const load = useCallback(async () => {
    const revision = ++requestRevision.current;
    setState((current) => ({ ...current, loading: true }));
    try {
      const entries = await getTimeline(date);
      if (revision !== requestRevision.current) return;
      setState({ entries, loading: false, error: null });
    } catch (error) {
      if (revision !== requestRevision.current) return;
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(error),
      }));
    }
  }, [date]);

  useEffect(() => {
    void load();
    window.addEventListener(ACTIVITY_DATA_CHANGED, load);
    return () => {
      window.removeEventListener(ACTIVITY_DATA_CHANGED, load);
    };
  }, [date, load]);

  return { ...state, refresh: () => void load() };
}
