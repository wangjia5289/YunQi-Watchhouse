import { describe, expect, it } from "vitest";

import {
  SETTINGS_TABS,
  createSettingsTabRequestDeduper,
  nextSettingsTab,
  settingsTabIsMounted,
} from "./settingsTabs";

describe("settings tabs", () => {
  it("keeps one panel mounted at a time", () => {
    for (const active of SETTINGS_TABS) {
      const mounted = SETTINGS_TABS.filter((tab) => settingsTabIsMounted(active.id, tab.id));
      expect(mounted.map((tab) => tab.id)).toEqual([active.id]);
    }
  });

  it("supports wrapping arrow-key navigation", () => {
    expect(nextSettingsTab("general", -1)).toBe("diagnostics-updates");
    expect(nextSettingsTab("general", 1)).toBe("classification");
    expect(nextSettingsTab("diagnostics-updates", 1)).toBe("general");
  });

  it("deduplicates only in-flight tab requests and allows refresh or retry", async () => {
    let finish: (() => void) | undefined;
    const calls: string[] = [];
    const load = createSettingsTabRequestDeduper((tab) => {
      calls.push(tab);
      return new Promise<void>((resolve) => {
        finish = resolve;
      });
    });

    const first = load("data-safety");
    expect(load("data-safety")).toBe(first);
    expect(calls).toEqual(["data-safety"]);
    finish?.();
    await first;

    expect(load("data-safety")).not.toBe(first);
    expect(calls).toEqual(["data-safety", "data-safety"]);
  });

  it("releases failed requests so re-entering a tab can retry", async () => {
    let attempts = 0;
    const load = createSettingsTabRequestDeduper(async () => {
      attempts += 1;
      if (attempts === 1) throw new Error("offline");
    });

    await expect(load("diagnostics-updates")).rejects.toThrow("offline");
    await expect(load("diagnostics-updates")).resolves.toBeUndefined();
    expect(attempts).toBe(2);
  });
});
