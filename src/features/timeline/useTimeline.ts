import { useCallback, useEffect, useState } from "react";
import { TimelineEntry, errorMessage, getTimeline } from "../../lib/ipc";
import { localIsoDate } from "../../lib/format";
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

  const load = useCallback(async () => {
    setState((current) => ({ ...current, loading: true }));
    try {
      const entries = await getTimeline(date);
      setState({ entries, loading: false, error: null });
    } catch (error) {
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
    const timer =
      date === localIsoDate() ? window.setInterval(() => void load(), 10_000) : null;
    return () => {
      if (timer !== null) window.clearInterval(timer);
      window.removeEventListener(ACTIVITY_DATA_CHANGED, load);
    };
  }, [date, load]);

  return { ...state, refresh: () => void load() };
}
