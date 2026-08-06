import type { TimelineEntry, TimelinePage } from "../../lib/ipc";

export function appendTimelinePage(
  currentEntries: TimelineEntry[],
  page: TimelinePage,
  expectedOffset: number,
): TimelineEntry[] | null {
  if (page.offset !== expectedOffset) return null;

  const seen = new Set(currentEntries.map((entry) => entry.sessionId));
  const nextEntries = page.entries.filter((entry) => {
    if (seen.has(entry.sessionId)) return false;
    seen.add(entry.sessionId);
    return true;
  });
  return nextEntries.length ? [...currentEntries, ...nextEntries] : currentEntries;
}
