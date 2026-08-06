import { describe, expect, it } from "vitest";
import type { TimelineEntry, TimelinePage } from "../../lib/ipc";
import { appendTimelinePage } from "./timelinePageModel";

function entry(sessionId: number): TimelineEntry {
  return {
    sessionId,
    applicationId: null,
    state: "IDLE",
    applicationName: null,
    bundleIdentifier: null,
    category: null,
    windowTitle: null,
    note: null,
    project: null,
    tags: [],
    startedAtMs: sessionId * 1_000,
    endedAtMs: sessionId * 1_000 + 500,
    durationMs: 500,
    isOpen: false,
  };
}

function page(offset: number, entries: TimelineEntry[]): TimelinePage {
  return {
    entries,
    totalCount: 4,
    activeDurationMs: 0,
    idleDurationMs: 2_000,
    offset,
    hasMore: false,
  };
}

describe("appendTimelinePage", () => {
  it("appends only unseen sessions for the expected offset", () => {
    const current = [entry(1), entry(2)];
    expect(appendTimelinePage(current, page(2, [entry(2), entry(3)]), 2))
      .toEqual([entry(1), entry(2), entry(3)]);
  });

  it("rejects a response for a different offset", () => {
    expect(appendTimelinePage([entry(1)], page(0, [entry(2)]), 1)).toBeNull();
  });
});
