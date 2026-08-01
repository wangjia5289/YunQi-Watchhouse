import { useEffect, useState } from "react";
import {
  BackupPreview,
  createEncryptedDatabaseBackup,
  clearAutomaticEncryptedBackupPassword,
  errorMessage,
  hasAutomaticEncryptedBackupPassword,
  previewEncryptedDatabaseRestore,
  restorePreparedDatabase,
  setAutomaticEncryptedBackupPassword,
} from "../../lib/ipc";
import { useLocale } from "../../lib/i18n";
import { notifyActivityDataChanged } from "../../lib/events";
import "./EncryptedBackupControls.css";
import { RestorePreview } from "./RestorePreview";

export function EncryptedBackupControls({
  onMessage,
  onRestored,
  automaticEnabled,
  onAutomaticEnabledChange,
}: {
  onMessage: (message: string) => void;
  onRestored: () => void;
  automaticEnabled: boolean;
  onAutomaticEnabledChange: (enabled: boolean) => Promise<void>;
}) {
  const { t } = useLocale();
  const [mode, setMode] = useState<"backup" | "restore">("backup");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);
  const [restorePreview, setRestorePreview] = useState<BackupPreview | null>(null);
  const [automaticPassword, setAutomaticPassword] = useState("");
  const [automaticPasswordSaved, setAutomaticPasswordSaved] = useState(false);
  const passwordValid = password.length >= 10;
  const canSubmit = passwordValid && (mode === "restore" || password === confirmation);

  useEffect(() => {
    void hasAutomaticEncryptedBackupPassword()
      .then(setAutomaticPasswordSaved)
      .catch(() => setAutomaticPasswordSaved(false));
  }, []);

  async function saveAutomaticPassword() {
    if (automaticPassword.length < 10) return;
    setBusy(true);
    try {
      await setAutomaticEncryptedBackupPassword(automaticPassword);
      setAutomaticPasswordSaved(true);
      setAutomaticPassword("");
      onMessage(t("Automatic encrypted backup password saved securely."));
    } catch (reason) {
      onMessage(t(errorMessage(reason)));
    } finally {
      setBusy(false);
    }
  }

  async function submit() {
    if (!canSubmit) return;
    setBusy(true);
    onMessage("");
    try {
      if (mode === "backup") {
        const path = await createEncryptedDatabaseBackup(password);
        if (path) onMessage(`${t("Encrypted backup saved to")} ${path}`);
      } else {
        const preview = await previewEncryptedDatabaseRestore(password);
        if (preview) setRestorePreview(preview);
      }
    } catch (reason) {
      onMessage(t(errorMessage(reason)));
    } finally {
      setPassword("");
      setConfirmation("");
      setBusy(false);
    }
  }

  function selectMode(nextMode: "backup" | "restore") {
    if (nextMode === mode) return;
    setRestorePreview(null);
    setPassword("");
    setConfirmation("");
    setMode(nextMode);
  }

  return (
    <div className="encrypted-backup-controls">
      <div className="encrypted-backup-heading">
        <div>
          <strong>{t("Encrypted backup")}</strong>
          <small>{t("Protect a portable backup with a password. The password is never stored.")}</small>
        </div>
        <div className="encrypted-backup-mode" role="group" aria-label={t("Encrypted backup action")}>
          <button type="button" className={mode === "backup" ? "active" : ""} onClick={() => selectMode("backup")}>
            {t("Create")}
          </button>
          <button type="button" className={mode === "restore" ? "active" : ""} onClick={() => selectMode("restore")}>
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
      {mode === "restore" && restorePreview && (
        <RestorePreview
          preview={restorePreview}
          busy={busy}
          onCancel={() => setRestorePreview(null)}
          onConfirm={() => {
            setBusy(true);
            void restorePreparedDatabase(restorePreview.token).then(() => {
              setRestorePreview(null);
              onMessage(t("Encrypted backup restored. Review the data, then resume tracking."));
              notifyActivityDataChanged();
              onRestored();
            }).catch((reason) => {
              setRestorePreview(null);
              onMessage(t(errorMessage(reason)));
            }).finally(() => setBusy(false));
          }}
        />
      )}
      <div className="encrypted-backup-automatic">
        <div>
          <strong>{t("Automatic encrypted backups")}</strong>
          <small>{t("The password is stored in the system Keychain, not in Watchhouse data.")}</small>
          <small>{t("Uses the backup schedule and retention settings below.")}</small>
        </div>
        <label className="encrypted-backup-automatic-toggle">
          <input
            type="checkbox"
            checked={automaticEnabled}
            disabled={busy || !automaticPasswordSaved}
            onChange={(event) => void onAutomaticEnabledChange(event.currentTarget.checked)}
          />
          <span>{t(automaticEnabled ? "Enabled" : "Disabled")}</span>
        </label>
        <div className="encrypted-backup-fields">
          <label>
            <span>{t(automaticPasswordSaved ? "Replace automatic backup password" : "Automatic backup password")}</span>
            <input
              type="password"
              autoComplete="new-password"
              value={automaticPassword}
              disabled={busy}
              onChange={(event) => setAutomaticPassword(event.currentTarget.value)}
            />
          </label>
          <button type="button" disabled={busy || automaticPassword.length < 10} onClick={() => void saveAutomaticPassword()}>
            {t("Save securely")}
          </button>
          {automaticPasswordSaved && (
            <button type="button" disabled={busy} onClick={() => {
              void (async () => {
                if (automaticEnabled) await onAutomaticEnabledChange(false);
                await clearAutomaticEncryptedBackupPassword();
                setAutomaticPasswordSaved(false);
                onMessage(t("Automatic encrypted backup password removed."));
              })().catch((reason) => onMessage(t(errorMessage(reason))));
            }}>
              {t("Remove password")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
