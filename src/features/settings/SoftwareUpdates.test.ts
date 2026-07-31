import { describe, expect, it } from "vitest";
import { UpdateCheck } from "../../lib/ipc";
import {
  INITIAL_SOFTWARE_UPDATE_STATE,
  reduceSoftwareUpdateState,
} from "./SoftwareUpdatesModel";

const availableUpdate: UpdateCheck = {
  configured: true,
  available: true,
  currentVersion: "0.1.0",
  version: "0.2.0",
  notes: null,
  publishedAt: null,
};

describe("reduceSoftwareUpdateState", () => {
  it("clears a stale available update when a new check starts or fails", () => {
    const previous = {
      ...INITIAL_SOFTWARE_UPDATE_STATE,
      result: availableUpdate,
    };
    const checking = reduceSoftwareUpdateState(previous, { type: "check-started" });
    expect(checking.result).toBeNull();
    expect(checking.checking).toBe(true);

    const failed = reduceSoftwareUpdateState(checking, {
      type: "check-failed",
      details: "network unavailable",
    });
    expect(failed.result).toBeNull();
    expect(failed.checking).toBe(false);
    expect(failed.error).toEqual({
      summary: "Update check failed.",
      details: "network unavailable",
    });
  });

  it("keeps installation errors separate from check errors", () => {
    const installing = reduceSoftwareUpdateState(
      { ...INITIAL_SOFTWARE_UPDATE_STATE, result: availableUpdate },
      { type: "install-started" },
    );
    const failed = reduceSoftwareUpdateState(installing, {
      type: "install-failed",
      details: "signature invalid",
    });
    expect(failed.installing).toBe(false);
    expect(failed.error?.summary).toBe("Update installation failed.");
    expect(failed.result).toEqual(availableUpdate);
  });
});
