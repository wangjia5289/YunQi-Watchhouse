import { describe, expect, it, vi } from "vitest";

import { createRetryableLoader, preloadSilently } from "./lazyLoader";

describe("createRetryableLoader", () => {
  it("shares one in-flight and completed load", async () => {
    const source = vi.fn(async () => ({ value: 42 }));
    const load = createRetryableLoader(source);

    const first = load();
    const second = load();

    expect(second).toBe(first);
    await expect(first).resolves.toEqual({ value: 42 });
    await expect(load()).resolves.toEqual({ value: 42 });
    expect(source).toHaveBeenCalledTimes(1);
  });

  it("allows a failed preload to be retried", async () => {
    const source = vi.fn()
      .mockRejectedValueOnce(new Error("temporary chunk failure"))
      .mockResolvedValueOnce({ value: 42 });
    const load = createRetryableLoader(source);

    await expect(load()).rejects.toThrow("temporary chunk failure");
    await expect(load()).resolves.toEqual({ value: 42 });
    expect(source).toHaveBeenCalledTimes(2);
  });

  it("handles preload failures without surfacing an unhandled rejection", async () => {
    const source = vi.fn().mockRejectedValue(new Error("offline"));
    const load = createRetryableLoader(source);

    preloadSilently(load);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(source).toHaveBeenCalledTimes(1);
  });
});
