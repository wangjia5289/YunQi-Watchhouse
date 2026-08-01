import {
  CurrentActivity,
  FocusModeStatus,
  UsageLimitProgress,
} from "../../lib/ipc";

export function currentApplicationName(current: CurrentActivity | null): string | null {
  if (!current || current.paused) return null;
  if (current.monitor.status !== "RUNNING") return null;
  return current.monitor.payload.foregroundApplication?.name ?? null;
}

export function closestUsageLimit(
  progress: UsageLimitProgress[],
): UsageLimitProgress | null {
  return progress
    .filter((item) => item.enabled)
    .reduce<UsageLimitProgress | null>((closest, item) => (
      closest === null || item.percentage > closest.percentage ? item : closest
    ), null);
}

export function usageLimitName(progress: UsageLimitProgress): string {
  return progress.applicationName ?? progress.category ?? "Watchhouse";
}

export function focusElapsedMs(status: FocusModeStatus | null, nowMs: number): number {
  if (!status?.active || status.startedAtMs === null) return 0;
  const currentPauseMs = status.paused && status.pausedAtMs !== null
    ? Math.max(0, nowMs - status.pausedAtMs)
    : 0;
  return Math.max(
    0,
    nowMs - status.startedAtMs - status.totalPausedMs - currentPauseMs,
  );
}

export function focusRemainingMs(status: FocusModeStatus | null, nowMs: number): number | null {
  if (!status?.active || status.plannedEndAtMs === null) return null;
  const currentPauseMs = status.paused && status.pausedAtMs !== null
    ? Math.max(0, nowMs - status.pausedAtMs)
    : 0;
  return Math.max(0, status.plannedEndAtMs + currentPauseMs - nowMs);
}
