import { afterEach, describe, expect, it } from "vitest";
import type { TimelineEntry } from "../../lib/ipc";
import { summarizeByHour } from "./timelineModel";

const originalTimezone = process.env.TZ;

afterEach(() => {
  if (originalTimezone === undefined) delete process.env.TZ;
  else process.env.TZ = originalTimezone;
});

function activeEntry(startedAtMs: number, endedAtMs: number): TimelineEntry {
  return {
    sessionId: 1,
    applicationId: 1,
    state: "ACTIVE",
    applicationName: "Test",
    bundleIdentifier: null,
    category: null,
    windowTitle: null,
    note: null,
    startedAtMs,
    endedAtMs,
    durationMs: endedAtMs - startedAtMs,
    isOpen: false,
  };
}

describe("timeline hourly summary", () => {
  it("advances through the repeated hour when daylight saving time ends", () => {
    process.env.TZ = "America/New_York";
    const startedAtMs = Date.parse("2025-11-02T01:30:00-05:00");
    const summaries = summarizeByHour([
      activeEntry(startedAtMs, startedAtMs + 90 * 60_000),
    ]);

    expect(summaries.map((summary) => new Date(summary.startedAtMs).getHours()))
      .toEqual([1, 2]);
    expect(summaries.reduce(
      (total, summary) => total + summary.activeDurationMs,
      0,
    )).toBe(90 * 60_000);
  });
});
