import { describe, expect, it } from "vitest";
import { buildWeeklyArchiveInput } from "./weeklyArchiveModel";

describe("buildWeeklyArchiveInput", () => {
  it("creates a stable seven-date archive snapshot", () => {
    const input = buildWeeklyArchiveInput({
      range: {
        startMs: new Date(2026, 6, 27).getTime(),
        endMs: new Date(2026, 6, 31, 12).getTime(),
      },
      activeDurationMs: 7_200_000,
      idleDurationMs: 600_000,
      previousActiveDurationMs: 0,
      previousIdleDurationMs: 0,
      dailyUsage: [{ date: "2026-07-29", activeDurationMs: 7_200_000, idleDurationMs: 0 }],
      hourlyUsage: [{ hour: 9, activeDurationMs: 7_200_000 }],
      categoryUsage: [{ category: "Development", durationMs: 7_200_000, applicationCount: 1 }],
      organizationInsights: {
        projectUsage: [],
        tagUsage: [],
        unassignedActiveDurationMs: 0,
        unassignedSessionCount: 0,
      },
    }, null, 3_600_000, 42);

    expect(input.weekStartDate).toBe("2026-07-27");
    expect(input.weekEndDate).toBe("2026-08-02");
    expect(input.strongestDayDate).toBe("2026-07-29");
    expect(input.peakHour).toBe(9);
    expect(input.leadingCategory).toBe("Development");
    expect(input.generatedAtMs).toBe(42);
    expect(JSON.parse(input.payloadJson).insights.topCategory.category).toBe("Development");
  });
});
