import { useCallback, useEffect, useRef, useState } from "react";
import {
  AppUsage,
  CategoryUsage,
  errorMessage,
  getAppUsage,
  getCategoryUsage,
} from "../../lib/ipc";
import { ACTIVITY_DATA_CHANGED } from "../../lib/events";

export interface ApplicationRange {
  startMs: number;
  endMs: number;
}

interface ApplicationsState {
  applications: AppUsage[];
  categories: CategoryUsage[];
  loading: boolean;
  error: string | null;
}

export function useApplications(
  range: ApplicationRange,
): ApplicationsState & { refresh: () => void } {
  const [state, setState] = useState<ApplicationsState>({
    applications: [],
    categories: [],
    loading: true,
    error: null,
  });
  const requestRevision = useRef(0);

  const load = useCallback(async () => {
    const revision = ++requestRevision.current;
    setState((current) => ({ ...current, loading: true }));
    try {
      const [applications, categories] = await Promise.all([
        getAppUsage(range.startMs, range.endMs),
        getCategoryUsage(range.startMs, range.endMs),
      ]);
      if (revision !== requestRevision.current) return;
      setState({ applications, categories, loading: false, error: null });
    } catch (error) {
      if (revision !== requestRevision.current) return;
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(error),
      }));
    }
  }, [range.endMs, range.startMs]);

  useEffect(() => {
    void load();
    window.addEventListener(ACTIVITY_DATA_CHANGED, load);
    return () => window.removeEventListener(ACTIVITY_DATA_CHANGED, load);
  }, [load]);

  return { ...state, refresh: () => void load() };
}
