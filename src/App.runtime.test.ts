import { describe, expect, it, vi } from "vitest";

import { createSingleFlight, trackingForPage } from "./App";
import { shouldRunFocusTicker } from "./features/dashboard/Dashboard";
import type { CurrentActivity } from "./lib/ipc";

describe("frontend runtime scheduling", () => {
  it("shares a tracking request while it is in flight and releases it after completion", async () => {
    let finish: ((value: number) => void) | undefined;
    const task = vi.fn(() => new Promise<number>((resolve) => {
      finish = resolve;
    }));
    const run = createSingleFlight(task);

    const first = run();
    const second = run();
    expect(second).toBe(first);
    expect(task).toHaveBeenCalledTimes(1);

    finish?.(1);
    await expect(first).resolves.toBe(1);
    const third = run();
    expect(task).toHaveBeenCalledTimes(2);
    expect(third).not.toBe(first);
  });

  it("releases a failed tracking request for a later retry", async () => {
    const task = vi.fn()
      .mockRejectedValueOnce(new Error("temporary failure"))
      .mockResolvedValueOnce(2);
    const run = createSingleFlight(task);

    await expect(run()).rejects.toThrow("temporary failure");
    await expect(run()).resolves.toBe(2);
    expect(task).toHaveBeenCalledTimes(2);
  });

  it("only passes live tracking state to the Today page", () => {
    const tracking: CurrentActivity = {
      monitor: { status: "PAUSED" },
      persistence: { status: "RUNNING" },
      paused: true,
    };

    expect(trackingForPage("today", tracking)).toBe(tracking);
    for (const page of [
      "timeline",
      "search",
      "applications",
      "history",
      "reports",
      "settings",
    ] as const) {
      expect(trackingForPage(page, tracking)).toBeNull();
    }
  });

  it("runs the focus ticker only for visible, active, unpaused focus", () => {
    expect(shouldRunFocusTicker({ active: true, paused: false }, "visible")).toBe(true);
    expect(shouldRunFocusTicker({ active: false, paused: false }, "visible")).toBe(false);
    expect(shouldRunFocusTicker({ active: true, paused: true }, "visible")).toBe(false);
    expect(shouldRunFocusTicker({ active: true, paused: false }, "hidden")).toBe(false);
    expect(shouldRunFocusTicker(null, "visible")).toBe(false);
  });
});
