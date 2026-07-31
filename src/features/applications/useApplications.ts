import { useCallback, useEffect, useState } from "react";
import { AppUsage, errorMessage, getAppUsage } from "../../lib/ipc";
import { ACTIVITY_DATA_CHANGED } from "../../lib/events";

export interface ApplicationRange {
  startMs: number;
  endMs: number;
}

interface ApplicationsState {
  applications: AppUsage[];
  loading: boolean;
  error: string | null;
}

export function useApplications(
  range: ApplicationRange,
): ApplicationsState & { refresh: () => void } {
  const [state, setState] = useState<ApplicationsState>({
    applications: [],
    loading: true,
    error: null,
  });

  const load = useCallback(async () => {
    setState((current) => ({ ...current, loading: true }));
    try {
      const applications = await getAppUsage(range.startMs, range.endMs);
      setState({ applications, loading: false, error: null });
    } catch (error) {
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
