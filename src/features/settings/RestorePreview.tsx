import { useEffect } from "react";
import { BackupPreview, cancelPreparedDatabaseRestore } from "../../lib/ipc";
import { useLocale } from "../../lib/i18n";
import "./RestorePreview.css";

export function RestorePreview({
  preview,
  busy,
  onConfirm,
  onCancel,
}: {
  preview: BackupPreview;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { locale, t } = useLocale();
  const dateLocale = locale === "zh-CN" ? "zh-CN" : "en-US";
  const formatDate = (value: number | null) => value === null
    ? t("No activity")
    : new Intl.DateTimeFormat(dateLocale, { dateStyle: "medium", timeStyle: "short" }).format(value);
  const formatBytes = (bytes: number) => new Intl.NumberFormat(dateLocale, {
    style: "unit",
    unit: bytes >= 1024 * 1024 ? "megabyte" : "kilobyte",
    maximumFractionDigits: 1,
  }).format(bytes / (bytes >= 1024 * 1024 ? 1024 * 1024 : 1024));

  useEffect(() => {
    const token = preview.token;
    return () => {
      void cancelPreparedDatabaseRestore(token);
    };
  }, [preview.token]);

  function cancel() {
    void cancelPreparedDatabaseRestore(preview.token);
    onCancel();
  }

  return (
    <div className="restore-preview" role="region" aria-label={t("Restore preview")}>
      <div className="restore-preview-title">
        <strong>{t("Restore preview")}</strong>
        <span>{preview.fileName}</span>
      </div>
      <dl>
        <div><dt>{t("Backup type")}</dt><dd>{t(preview.encrypted ? "Encrypted" : "SQLite")}</dd></div>
        <div><dt>{t("Schema version")}</dt><dd>{t("Version")} {preview.schemaVersion}</dd></div>
        <div><dt>{t("File size")}</dt><dd>{formatBytes(preview.fileSizeBytes)}</dd></div>
        <div><dt>{t("Applications")}</dt><dd>{preview.applicationCount.toLocaleString(dateLocale)}</dd></div>
        <div><dt>{t("Sessions")}</dt><dd>{preview.sessionCount.toLocaleString(dateLocale)}</dd></div>
        <div><dt>{t("Weekly reports")}</dt><dd>{preview.weeklyReportCount.toLocaleString(dateLocale)}</dd></div>
        <div className="restore-preview-wide"><dt>{t("Activity range")}</dt><dd>{formatDate(preview.earliestSessionAtMs)} - {formatDate(preview.latestSessionAtMs)}</dd></div>
      </dl>
      <p>{t("Confirming will replace current activity data and pause tracking.")}</p>
      <div className="restore-preview-actions">
        <button type="button" disabled={busy} onClick={cancel}>{t("Cancel")}</button>
        <button type="button" className="danger-action" disabled={busy} onClick={onConfirm}>
          {t(busy ? "Restoring…" : "Confirm restore")}
        </button>
      </div>
    </div>
  );
}
