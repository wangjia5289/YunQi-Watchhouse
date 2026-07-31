import type { TimelineEntry } from "../../lib/ipc";

interface HourApplication {
  id: number;
  name: string;
  durationMs: number;
}

export interface HourSummary {
  startedAtMs: number;
  activeDurationMs: number;
  idleDurationMs: number;
  sessionCount: number;
  applications: HourApplication[];
}

const HOUR_MS = 60 * 60 * 1_000;

export function summarizeByHour(entries: TimelineEntry[]): HourSummary[] {
  const groups = new Map<number, {
    activeDurationMs: number;
    idleDurationMs: number;
    sessionIds: Set<number>;
    applications: Map<number, HourApplication>;
  }>();

  for (const entry of entries) {
    let cursor = entry.startedAtMs;
    while (cursor < entry.endedAtMs) {
      const hour = new Date(cursor);
      hour.setMinutes(0, 0, 0);
      let hourStart = hour.getTime();
      while (hourStart + HOUR_MS <= cursor) hourStart += HOUR_MS;
      const segmentEnd = Math.min(entry.endedAtMs, hourStart + HOUR_MS);
      const durationMs = segmentEnd - cursor;
      const group = groups.get(hourStart) ?? {
        activeDurationMs: 0,
        idleDurationMs: 0,
        sessionIds: new Set<number>(),
        applications: new Map<number, HourApplication>(),
      };
      group.sessionIds.add(entry.sessionId);
      if (entry.state === "IDLE") {
        group.idleDurationMs += durationMs;
      } else {
        group.activeDurationMs += durationMs;
        if (entry.applicationId !== null) {
          const application = group.applications.get(entry.applicationId) ?? {
            id: entry.applicationId,
            name: entry.applicationName ?? "Unknown application",
            durationMs: 0,
          };
          application.durationMs += durationMs;
          group.applications.set(entry.applicationId, application);
        }
      }
      groups.set(hourStart, group);
      cursor = segmentEnd;
    }
  }

  return [...groups.entries()]
    .sort(([left], [right]) => left - right)
    .map(([startedAtMs, group]) => ({
      startedAtMs,
      activeDurationMs: group.activeDurationMs,
      idleDurationMs: group.idleDurationMs,
      sessionCount: group.sessionIds.size,
      applications: [...group.applications.values()]
        .sort((left, right) => right.durationMs - left.durationMs)
        .slice(0, 3),
    }));
}
