import type { TimelineEntry } from "../../lib/ipc";
import { localIsoDate, shiftLocalDate } from "../../lib/format";

export type RangePreset = "7_DAYS" | "30_DAYS" | "CUSTOM";

export function rangeForPreset(
  preset: Exclude<RangePreset, "CUSTOM">,
  today: string,
) {
  return {
    startDate: shiftLocalDate(today, preset === "7_DAYS" ? -6 : -29),
    endDate: today,
  };
}

export function inclusiveDayCount(startDate: string, endDate: string): number {
  const toUtcDay = (value: string) => {
    const [year, month, day] = value.split("-").map(Number);
    return Date.UTC(year, month - 1, day);
  };
  return Math.floor((toUtcDay(endDate) - toUtcDay(startDate)) / 86_400_000) + 1;
}

export function groupEntriesByDate(entries: TimelineEntry[]) {
  const groups = new Map<string, TimelineEntry[]>();
  for (const entry of entries) {
    const date = localIsoDate(new Date(entry.startedAtMs));
    const group = groups.get(date) ?? [];
    group.push(entry);
    groups.set(date, group);
  }
  return [...groups.entries()]
    .sort(([left], [right]) => right.localeCompare(left))
    .map(([date, group]) => [
      date,
      group.sort((left, right) => right.startedAtMs - left.startedAtMs),
    ] as [string, TimelineEntry[]]);
}
