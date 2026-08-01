import { useState } from "react";
import {
  createEncryptedDatabaseBackup,
  errorMessage,
  restoreEncryptedDatabaseBackup,
} from "../../lib/ipc";
import { useLocale } from "../../lib/i18n";
import { notifyActivityDataChanged } from "../../lib/events";
import "./EncryptedBackupControls.css";

export function EncryptedBackupControls({
  onMessage,
  onRestored,
}: {
  onMessage: (message: string) => void;
  onRestored: () => void;
}) {
  const { t } = useLocale();
  const [mode, setMode] = useState<"backup" | "restore">("backup");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);
  const passwordValid = password.length >= 10;
  const canSubmit = passwordValid && (mode === "restore" || password === confirmation);

  async function submit() {
    if (!canSubmit) return;
    setBusy(true);
    onMessage("");
    try {
      if (mode === "backup") {
        const path = await createEncryptedDatabaseBackup(password);
        if (path) onMessage(`${t("Encrypted backup saved to")} ${path}`);
      } else if (window.confirm(t("Restore this encrypted backup? Current activity data will be replaced and tracking will pause."))) {
        const restored = await restoreEncryptedDatabaseBackup(password);
        if (restored) {
          onMessage(t("Encrypted backup restored. Review the data, then resume tracking."));
          notifyActivityDataChanged();
          onRestored();
        }
      }
    } catch (reason) {
      onMessage(t(errorMessage(reason)));
    } finally {
      setPassword("");
      setConfirmation("");
      setBusy(false);
    }
  }

  return (
    <div className="encrypted-backup-controls">
      <div className="encrypted-backup-heading">
        <div>
          <strong>{t("Encrypted backup")}</strong>
          <small>{t("Protect a portable backup with a password. The password is never stored.")}</small>
        </div>
        <div className="encrypted-backup-mode" role="group" aria-label={t("Encrypted backup action")}>
          <button type="button" className={mode === "backup" ? "active" : ""} onClick={() => setMode("backup")}>
            {t("Create")}
          </button>
          <button type="button" className={mode === "restore" ? "active" : ""} onClick={() => setMode("restore")}>
            {t("Restore")}
          </button>
        </div>
      </div>
      <div className="encrypted-backup-fields">
        <label>
          <span>{t("Backup password")}</span>
          <input
            type="password"
            autoComplete="new-password"
            value={password}
            disabled={busy}
            onChange={(event) => setPassword(event.currentTarget.value)}
          />
        </label>
        {mode === "backup" && (
          <label>
            <span>{t("Confirm password")}</span>
            <input
              type="password"
              autoComplete="new-password"
              value={confirmation}
              disabled={busy}
              onChange={(event) => setConfirmation(event.currentTarget.value)}
            />
          </label>
        )}
        <button type="button" disabled={busy || !canSubmit} onClick={() => void submit()}>
          {t(busy ? "Working…" : mode === "backup" ? "Create encrypted backup" : "Restore encrypted backup")}
        </button>
      </div>
      {!passwordValid && password.length > 0 && (
        <small className="encrypted-backup-validation">{t("Use at least 10 characters.")}</small>
      )}
      {mode === "backup" && confirmation.length > 0 && password !== confirmation && (
        <small className="encrypted-backup-validation">{t("Passwords do not match.")}</small>
      )}
    </div>
  );
}
