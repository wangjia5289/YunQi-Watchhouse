import { describe, expect, it } from "vitest";
import type { TimelineEntry } from "../../lib/ipc";
import {
  groupEntriesByDate,
  inclusiveDayCount,
  rangeForPreset,
} from "./searchModel";

function entry(sessionId: number, startedAtMs: number): TimelineEntry {
  return {
    sessionId,
    applicationId: sessionId,
    state: "ACTIVE",
    applicationName: "Test",
    bundleIdentifier: null,
    category: null,
    windowTitle: null,
    note: null,
    startedAtMs,
    endedAtMs: startedAtMs + 1_000,
    durationMs: 1_000,
    isOpen: false,
  };
}

describe("global search date model", () => {
  it("builds inclusive rolling ranges across month boundaries", () => {
    expect(rangeForPreset("7_DAYS", "2026-03-03")).toEqual({
      startDate: "2026-02-25",
      endDate: "2026-03-03",
    });
    expect(rangeForPreset("30_DAYS", "2024-03-01")).toEqual({
      startDate: "2024-02-01",
      endDate: "2024-03-01",
    });
  });

  it("counts calendar days independently of daylight-saving offsets", () => {
    expect(inclusiveDayCount("2026-03-01", "2026-03-01")).toBe(1);
    expect(inclusiveDayCount("2024-01-01", "2024-12-31")).toBe(366);
    expect(inclusiveDayCount("2024-01-01", "2025-01-01")).toBe(367);
  });

  it("groups loaded entries by local date with newest activity first", () => {
    const first = new Date(2026, 6, 30, 9).getTime();
    const second = new Date(2026, 6, 31, 9).getTime();
    const groups = groupEntriesByDate([
      entry(1, first),
      entry(2, second),
      entry(3, first + 60_000),
    ]);

    expect(groups.map(([date]) => date)).toEqual(["2026-07-31", "2026-07-30"]);
    expect(groups[1][1].map((item) => item.sessionId)).toEqual([3, 1]);
  });
});
