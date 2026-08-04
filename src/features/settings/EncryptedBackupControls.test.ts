import { describe, expect, it, vi } from "vitest";
import {
  canToggleAutomaticEncryptedBackup,
  removeAutomaticEncryptedBackupPassword,
} from "./EncryptedBackupControls";

describe("automatic encrypted backup credentials", () => {
  it("keeps an enabled toggle operable when the password cannot be loaded", () => {
    expect(canToggleAutomaticEncryptedBackup(true, false)).toBe(true);
    expect(canToggleAutomaticEncryptedBackup(false, false)).toBe(false);
  });

  it("does not clear the password when disabling automatic backups fails", async () => {
    const disableAutomaticBackups = vi.fn().mockResolvedValue(false);
    const clearPassword = vi.fn().mockResolvedValue(undefined);

    await expect(removeAutomaticEncryptedBackupPassword(
      true,
      disableAutomaticBackups,
      clearPassword,
    )).resolves.toBe(false);
    expect(disableAutomaticBackups).toHaveBeenCalledOnce();
    expect(clearPassword).not.toHaveBeenCalled();
  });

  it("clears the password only after automatic backups are disabled", async () => {
    const calls: string[] = [];

    await expect(removeAutomaticEncryptedBackupPassword(
      true,
      async () => {
        calls.push("disable");
        return true;
      },
      async () => {
        calls.push("clear");
      },
    )).resolves.toBe(true);
    expect(calls).toEqual(["disable", "clear"]);
  });
});
