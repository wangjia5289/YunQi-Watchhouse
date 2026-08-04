import { describe, expect, it } from "vitest";
import { resolveRootTarget } from "./rootTarget";

describe("resolveRootTarget", () => {
  it("keeps browsers and the primary window on the main entry", () => {
    expect(resolveRootTarget(false)).toBe("main");
    expect(resolveRootTarget(true, "main")).toBe("main");
  });

  it("selects the isolated tray entry from the existing window label", () => {
    expect(resolveRootTarget(true, "tray-panel")).toBe("tray-panel");
  });
});
