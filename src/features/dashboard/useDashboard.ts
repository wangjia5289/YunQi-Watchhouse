import { useCallback, useEffect, useRef, useState } from "react";
import {
  FocusSummary,
  TimelineEntry,
  TodaySummary,
  UsageLimitProgress,
  errorMessage,
  getTimeline,
  getTodayFocusSummary,
  getTodayUsageLimitProgress,
  getTodaySummary,
} from "../../lib/ipc";
import { localIsoDate } from "../../lib/format";
import { ACTIVITY_DATA_CHANGED } from "../../lib/events";

interface DashboardState {
  summary: TodaySummary | null;
  timeline: TimelineEntry[];
  focus: FocusSummary | null;
  usageLimits: UsageLimitProgress[];
  loading: boolean;
  error: string | null;
}

const initialState: DashboardState = {
  summary: null,
  timeline: [],
  focus: null,
  usageLimits: [],
  loading: true,
  error: null,
};

export function useDashboard(): DashboardState & { refresh: () => void } {
  const [state, setState] = useState(initialState);
  const historyRequest = useRef(0);

  const loadHistory = useCallback(async () => {
    const request = ++historyRequest.current;
    try {
      const [summary, timeline, focus, usageLimits] = await Promise.all([
        getTodaySummary(),
        getTimeline(localIsoDate()),
        getTodayFocusSummary(),
        getTodayUsageLimitProgress(),
      ]);
      if (request !== historyRequest.current) return;
      setState((current) => ({
        ...current,
        summary,
        timeline,
        focus,
        usageLimits,
        loading: false,
        error: null,
      }));
    } catch (error) {
      if (request !== historyRequest.current) return;
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(error),
      }));
    }
  }, []);

  useEffect(() => {
    void loadHistory();
    const reload = () => void loadHistory();
    window.addEventListener(ACTIVITY_DATA_CHANGED, reload);
    return () => {
      window.removeEventListener(ACTIVITY_DATA_CHANGED, reload);
    };
  }, [loadHistory]);

  return {
    ...state,
    refresh: () => void loadHistory(),
  };
}
