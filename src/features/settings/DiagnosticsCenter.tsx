import { useState } from "react";
import {
  DiagnosticsSummary,
  errorMessage,
  runDiagnosticsRepair,
} from "../../lib/ipc";
import { useLocale } from "../../lib/i18n";
import { notifyActivityDataChanged } from "../../lib/events";
import "./DiagnosticsCenter.css";

type DiagnosticState = "healthy" | "attention" | "neutral";

function DiagnosticItem({
  label,
  value,
  state,
}: {
  label: string;
  value: string;
  state: DiagnosticState;
}) {
  return (
    <div className={`diagnostic-item ${state}`}>
      <i aria-hidden="true" />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

export function DiagnosticsCenter({
  diagnostics,
  onRefresh,
  onMessage,
}: {
  diagnostics: DiagnosticsSummary | null;
  onRefresh: () => Promise<void>;
  onMessage: (message: string) => void;
}) {
  const { locale, t } = useLocale();
  const [repairing, setRepairing] = useState(false);

  async function repair() {
    if (!window.confirm(t("Run safe diagnostics repair? Watchhouse will create a backup, repair session data, optimize the database, and refresh icons."))) return;
    setRepairing(true);
    onMessage("");
    try {
      const result = await runDiagnosticsRepair();
      notifyActivityDataChanged();
      await onRefresh();
      onMessage(result.databaseOptimized && result.iconCacheCleared
        ? t(`Diagnostics repair complete: ${result.trimmedSessionCount} adjusted, ${result.deletedSessionCount} removed. Safety backup: ${result.backupPath}`)
        : t(`Session repair completed, but some maintenance steps need attention. Safety backup: ${result.backupPath}`));
    } catch (reason) {
      onMessage(t(errorMessage(reason)));
    } finally {
      setRepairing(false);
    }
  }

  const backupValue = diagnostics?.automaticBackupEnabled
    ? diagnostics.lastBackupAtMs > 0
      ? new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(diagnostics.lastBackupAtMs)
      : t("Waiting for first backup")
    : t("Disabled");

  return (
    <section className="settings-card diagnostics-center" aria-labelledby="diagnostics-title">
      <div className="list-heading">
        <div><p className="section-kicker">{t("Health")}</p><h2 id="diagnostics-title">{t("Diagnostics center")}</h2></div>
        <button type="button" disabled={repairing || !diagnostics} onClick={() => void repair()}>
          {t(repairing ? "Repairing…" : "Run safe repair")}
        </button>
      </div>
      <p className="settings-note">{t("Review local services and repair common storage issues after creating a safety backup.")}</p>
      {diagnostics ? (
        <div className="diagnostic-list">
          <DiagnosticItem
            label={t("Database integrity")}
            value={t(diagnostics.databaseIntegrityOk ? "Healthy" : "Needs attention")}
            state={diagnostics.databaseIntegrityOk ? "healthy" : "attention"}
          />
          <DiagnosticItem
            label={t("Accessibility permission")}
            value={t(diagnostics.accessibilityPermission === "GRANTED" ? "Allowed" : diagnostics.accessibilityPermission === "UNSUPPORTED" ? "Unsupported" : "Denied")}
            state={diagnostics.accessibilityPermission === "GRANTED" ? "healthy" : "attention"}
          />
          <DiagnosticItem
            label={t("Notification permission")}
            value={t(diagnostics.notificationPermission === "GRANTED" ? "Allowed" : diagnostics.notificationPermission === "PROMPT" ? "Not requested" : "Denied")}
            state={diagnostics.notificationPermission === "GRANTED" ? "healthy" : "attention"}
          />
          <DiagnosticItem
            label={t("Activity tracking")}
            value={t(diagnostics.trackingPaused ? "Paused" : "Running")}
            state={diagnostics.trackingPaused ? "neutral" : "healthy"}
          />
          <DiagnosticItem
            label={t("Automatic backups")}
            value={backupValue}
            state={diagnostics.automaticBackupEnabled && diagnostics.backupDirectoryAvailable ? "healthy" : "neutral"}
          />
          <DiagnosticItem
            label={t("Diagnostic logs")}
            value={t(diagnostics.logDirectoryAvailable ? "Available" : "Unavailable")}
            state={diagnostics.logDirectoryAvailable ? "healthy" : "attention"}
          />
        </div>
      ) : <p className="settings-note">{t("Loading diagnostics…")}</p>}
      {diagnostics?.maintenanceLastError && (
        <p className="diagnostic-error" role="alert">{t("Last maintenance error:")} {diagnostics.maintenanceLastError}</p>
      )}
    </section>
  );
}
