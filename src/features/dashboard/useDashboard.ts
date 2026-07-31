import { useCallback, useEffect, useRef, useState } from "react";
import {
  CurrentActivity,
  TimelineEntry,
  TodaySummary,
  errorMessage,
  getCurrentActivity,
  getTimeline,
  getTodaySummary,
} from "../../lib/ipc";
import { localIsoDate } from "../../lib/format";
import { ACTIVITY_DATA_CHANGED } from "../../lib/events";

interface DashboardState {
  summary: TodaySummary | null;
  timeline: TimelineEntry[];
  current: CurrentActivity | null;
  loading: boolean;
  error: string | null;
}

const initialState: DashboardState = {
  summary: null,
  timeline: [],
  current: null,
  loading: true,
  error: null,
};

export function useDashboard(): DashboardState & { refresh: () => void } {
  const [state, setState] = useState(initialState);
  const historyRequest = useRef(0);
  const currentRequest = useRef(0);

  const loadHistory = useCallback(async () => {
    const request = ++historyRequest.current;
    try {
      const [summary, timeline] = await Promise.all([
        getTodaySummary(),
        getTimeline(localIsoDate()),
      ]);
      if (request !== historyRequest.current) return;
      setState((current) => ({
        ...current,
        summary,
        timeline,
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

  const loadCurrent = useCallback(async () => {
    const request = ++currentRequest.current;
    try {
      const currentActivity = await getCurrentActivity();
      if (request !== currentRequest.current) return;
      setState((current) => ({
        ...current,
        current: currentActivity,
      }));
    } catch (error) {
      if (request !== currentRequest.current) return;
      setState((current) => ({
        ...current,
        error: current.error ?? errorMessage(error),
      }));
    }
  }, []);

  useEffect(() => {
    void loadHistory();
    void loadCurrent();
    const historyTimer = window.setInterval(() => void loadHistory(), 10_000);
    const currentTimer = window.setInterval(() => void loadCurrent(), 2_000);
    const reload = () => {
      void loadHistory();
      void loadCurrent();
    };
    window.addEventListener(ACTIVITY_DATA_CHANGED, reload);
    return () => {
      window.clearInterval(historyTimer);
      window.clearInterval(currentTimer);
      window.removeEventListener(ACTIVITY_DATA_CHANGED, reload);
    };
  }, [loadCurrent, loadHistory]);

  return {
    ...state,
    refresh: () => {
      void loadHistory();
      void loadCurrent();
    },
  };
}
