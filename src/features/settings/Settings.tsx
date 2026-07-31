import { useEffect, useRef, useState } from "react";
import {
  DiagnosticsSummary,
  DataHealthSummary,
  DataHealthUndoStatus,
  MaintenancePreview,
  MaintenanceStatus,
  Settings as SettingsModel,
  ShortcutSettings,
  backupDatabase,
  chooseBackupDirectory,
  clearApplicationIconCache,
  createAutomaticBackupNow,
  deleteAllActivity,
  exportActivity,
  errorMessage,
  getSettings,
  getShortcutSettings,
  getAccessibilityPermission,
  AccessibilityPermission,
  NotificationPermission,
  getDiagnosticsSummary,
  getDataHealthSummary,
  getDataHealthUndoStatus,
  getMaintenancePreview,
  getMaintenanceStatus,
  getNotificationPermission,
  openDataDirectory,
  openBackupDirectory,
  openLogDirectory,
  optimizeDatabase,
  restoreDatabase,
  runDataMaintenance,
  requestNotificationPermission,
  repairDataHealth,
  undoDataHealthRepair,
  sendTestNotification,
  updateSettings,
  updateShortcutSettings,
} from "../../lib/ipc";
import { useLocale } from "../../lib/i18n";
import { notifyActivityDataChanged } from "../../lib/events";
import { clearApplicationIconMemoryCache } from "../applications/ApplicationIcon";
import { PrivacyNotice } from "../onboarding/PrivacyNotice";
import { CategoryRules } from "./CategoryRules";
import { SoftwareUpdates } from "./SoftwareUpdates";
import { UsageLimits } from "./UsageLimits";
import { UsageLimitReminderCenter } from "./UsageLimitReminderCenter";
import "./SettingsAnime.css";

function Toggle({
  checked,
  disabled,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      className={`settings-toggle${checked ? " active" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  );
}

export function Settings() {
  const { locale, t } = useLocale();
  const [settings, setSettings] = useState<SettingsModel | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [showPrivacy, setShowPrivacy] = useState(false);
  const [diagnostics, setDiagnostics] = useState<DiagnosticsSummary | null>(null);
  const [dataHealth, setDataHealth] = useState<DataHealthSummary | null>(null);
  const [dataHealthUndo, setDataHealthUndo] = useState<DataHealthUndoStatus | null>(null);
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const [maintenancePreview, setMaintenancePreview] = useState<MaintenancePreview | null>(null);
  const [maintenanceStatus, setMaintenanceStatus] = useState<MaintenanceStatus | null>(null);
  const [accessibilityPermission, setAccessibilityPermission] =
    useState<AccessibilityPermission>("UNSUPPORTED");
  const [notificationPermission, setNotificationPermission] =
    useState<NotificationPermission | null>(null);
  const [checkingNotification, setCheckingNotification] = useState(false);
  const [shortcuts, setShortcuts] = useState<ShortcutSettings | null>(null);
  const [savingShortcuts, setSavingShortcuts] = useState(false);

  useEffect(() => {
    void getSettings()
      .then(setSettings)
      .catch((error) => setMessage(errorMessage(error)));
    void getDiagnosticsSummary().then(setDiagnostics);
    void getDataHealthSummary().then(setDataHealth);
    void getDataHealthUndoStatus().then(setDataHealthUndo);
    void getMaintenancePreview().then(setMaintenancePreview);
    void getMaintenanceStatus().then(setMaintenanceStatus);
    void getAccessibilityPermission().then(setAccessibilityPermission);
    void getNotificationPermission()
      .then(setNotificationPermission)
      .catch((error) => setMessage(errorMessage(error)));
    void getShortcutSettings()
      .then(setShortcuts)
      .catch((error) => setMessage(errorMessage(error)));
  }, []);

  useEffect(() => {
    if (settings) {
      document.documentElement.dataset.theme = settings.appearance.toLowerCase();
    }
  }, [settings?.appearance]);

  useEffect(() => {
    if (settings) void getMaintenancePreview().then(setMaintenancePreview);
  }, [settings?.retentionDays]);

  async function save(next: SettingsModel) {
    if (savingRef.current) return;
    savingRef.current = true;
    setSaving(true);
    setSettings(next);
    try {
      setSettings(await updateSettings(next));
      setMessage("Settings saved.");
    } catch (error) {
      setMessage(errorMessage(error));
      try {
        setSettings(await getSettings());
      } catch (reloadError) {
        setMessage(errorMessage(reloadError));
      }
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  }

  if (!settings) {
    return <div className="settings-page"><p>{t("Loading settings…")}</p></div>;
  }

  return (
    <div className="settings-page">
      <header><div><p className="date-label">{t("Preferences")}</p><h1>{t("Settings")}</h1></div></header>
      {message && <div className="settings-message">{t(message)}</div>}

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">{t("General")}</p><h2>{t("Application")}</h2></div></div>
        <div className="setting-row">
          <div><strong>{t("Launch at Login")}</strong><small>{t("Start Watchhouse when you sign in.")}</small></div>
          <Toggle checked={settings.launchAtLogin} disabled={saving}
            onChange={(value) => void save({ ...settings, launchAtLogin: value })} />
        </div>
        <div className="setting-row">
          <div><strong>{t("Hide to Tray on Close")}</strong><small>{t("Keep tracking when the window closes.")}</small></div>
          <Toggle checked={settings.hideToTrayOnClose} disabled={saving}
            onChange={(value) => void save({ ...settings, hideToTrayOnClose: value })} />
        </div>
        <div className="setting-row">
          <div><strong>{t("Start Tracking Automatically")}</strong><small>{t("Begin recording when Watchhouse starts.")}</small></div>
          <Toggle checked={settings.startTrackingAutomatically} disabled={saving}
            onChange={(value) => void save({ ...settings, startTrackingAutomatically: value })} />
        </div>
      </section>

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">{t("Tracking")}</p><h2>{t("Activity detection")}</h2></div></div>
        <label className="setting-row">
          <div><strong>{t("Idle Threshold")}</strong><small>{t("Changes apply immediately.")}</small></div>
          <select value={settings.idleThresholdSeconds} disabled={saving}
            onChange={(event) => void save({ ...settings, idleThresholdSeconds: Number(event.currentTarget.value) })}>
            <option value={60}>{t("1 minute")}</option><option value={180}>{t("3 minutes")}</option>
            <option value={300}>{t("5 minutes")}</option><option value={600}>{t("10 minutes")}</option>
            <option value={900}>{t("15 minutes")}</option>
          </select>
        </label>
        <div className="setting-row">
          <div>
            <strong>{t("Record Window Titles")}</strong>
            <small>
              {t(accessibilityPermission === "GRANTED"
                ? "Off by default for every application; sensitive text is redacted locally."
                : "Requires macOS Accessibility permission. Enable it in System Settings, then reopen Watchhouse.")}
            </small>
          </div>
          <Toggle
            checked={settings.recordWindowTitles}
            disabled={saving || accessibilityPermission !== "GRANTED"}
            onChange={(value) => void save({ ...settings, recordWindowTitles: value })}
          />
        </div>
      </section>

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">{t("Appearance")}</p><h2>{t("Theme")}</h2></div></div>
        <div className="settings-segmented">
          {(["SYSTEM", "LIGHT", "DARK"] as const).map((value) => (
            <button key={value} disabled={saving}
              className={settings.appearance === value ? "active" : ""}
              onClick={() => void save({ ...settings, appearance: value })}>
              {t(value[0] + value.slice(1).toLowerCase())}
            </button>
          ))}
        </div>
      </section>

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">{t("Focus")}</p><h2>{t("Goals and breaks")}</h2></div></div>
        <label className="setting-row">
          <div><strong>{t("Daily Focus Goal")}</strong><small>{t("Progress appears on Today.")}</small></div>
          <select
            value={settings.dailyFocusGoalMinutes}
            disabled={saving}
            onChange={(event) => void save({
              ...settings,
              dailyFocusGoalMinutes: Number(event.currentTarget.value),
            })}
          >
            <option value={0}>{t("Off")}</option>
            <option value={120}>{t("2 hours")}</option>
            <option value={240}>{t("4 hours")}</option>
            <option value={360}>{t("6 hours")}</option>
            <option value={480}>{t("8 hours")}</option>
          </select>
        </label>
        <label className="setting-row">
          <div><strong>{t("Focus Block Gap")}</strong><small>{t("Longer idle periods end a focus block.")}</small></div>
          <select
            value={settings.focusBlockGapMinutes}
            disabled={saving}
            onChange={(event) => void save({
              ...settings,
              focusBlockGapMinutes: Number(event.currentTarget.value),
            })}
          >
            {[3, 5, 10, 15, 30].map((minutes) => (
              <option value={minutes} key={minutes}>{t(`${minutes} minutes`)}</option>
            ))}
          </select>
        </label>
        <div className="setting-row">
          <div><strong>{t("Break Reminders")}</strong><small>{t("Show a local reminder after continuous focus.")}</small></div>
          <Toggle
            checked={settings.breakRemindersEnabled}
            disabled={saving}
            onChange={(value) => void save({ ...settings, breakRemindersEnabled: value })}
          />
        </div>
        <div className="setting-row notification-permission-row">
          <div>
            <strong>{t("Notification Permission")}</strong>
            <small>
              {t(notificationPermission === "GRANTED"
                ? "Allowed. Use a test notification to verify macOS delivery settings."
                : notificationPermission === "DENIED"
                  ? "Denied. Allow Watchhouse notifications in macOS System Settings."
                  : notificationPermission === "PROMPT"
                    ? "Permission has not been requested."
                    : "Checking notification permission...")}
            </small>
          </div>
          <div className="notification-actions">
            <span
              className={`notification-status ${notificationPermission?.toLowerCase() ?? "checking"}`}
            >
              {t(notificationPermission === "GRANTED"
                ? "Allowed"
                : notificationPermission === "DENIED"
                  ? "Denied"
                  : notificationPermission === "PROMPT"
                    ? "Not requested"
                    : "Checking")}
            </span>
            {notificationPermission === "PROMPT" && (
              <button
                type="button"
                disabled={checkingNotification}
                onClick={() => {
                  setCheckingNotification(true);
                  void requestNotificationPermission()
                    .then((permission) => {
                      setNotificationPermission(permission);
                      setMessage(
                        permission === "GRANTED"
                          ? "Notification permission granted."
                          : "Notification permission was not granted.",
                      );
                    })
                    .catch((error) => setMessage(errorMessage(error)))
                    .finally(() => setCheckingNotification(false));
                }}
              >
                {t("Allow")}
              </button>
            )}
            {notificationPermission === "DENIED" && (
              <button
                type="button"
                disabled={checkingNotification}
                onClick={() => {
                  setCheckingNotification(true);
                  void getNotificationPermission()
                    .then(setNotificationPermission)
                    .catch((error) => setMessage(errorMessage(error)))
                    .finally(() => setCheckingNotification(false));
                }}
              >
                {t("Check Again")}
              </button>
            )}
            <button
              type="button"
              disabled={checkingNotification || notificationPermission !== "GRANTED"}
              onClick={() => {
                setCheckingNotification(true);
                void sendTestNotification()
                  .then(() => setMessage("Test notification sent."))
                  .catch((error) => setMessage(errorMessage(error)))
                  .finally(() => setCheckingNotification(false));
              }}
            >
              {t("Send Test")}
            </button>
          </div>
        </div>
        <label className="setting-row">
          <div><strong>{t("Reminder Interval")}</strong><small>{t("Reminders reset when a new focus block starts.")}</small></div>
          <select
            value={settings.breakReminderMinutes}
            disabled={saving || !settings.breakRemindersEnabled}
            onChange={(event) => void save({
              ...settings,
              breakReminderMinutes: Number(event.currentTarget.value) as SettingsModel["breakReminderMinutes"],
            })}
          >
            {[30, 45, 60, 90, 120].map((minutes) => (
              <option value={minutes} key={minutes}>{t(`${minutes} minutes`)}</option>
            ))}
          </select>
        </label>
        <div className="setting-row">
          <div><strong>{t("Quiet Hours")}</strong><small>{t("Break and usage limit reminders stay silent during this period.")}</small></div>
          <span className="quiet-hours">
            <input
              type="time"
              value={settings.quietHoursStart}
              disabled={saving}
              onChange={(event) => void save({ ...settings, quietHoursStart: event.currentTarget.value })}
            />
            <span>{t("to")}</span>
            <input
              type="time"
              value={settings.quietHoursEnd}
              disabled={saving}
              onChange={(event) => void save({ ...settings, quietHoursEnd: event.currentTarget.value })}
            />
          </span>
        </div>
      </section>

      <CategoryRules />
      <UsageLimits />
      <UsageLimitReminderCenter />

      {shortcuts && (
        <section className="settings-card">
          <div className="list-heading">
            <div><p className="section-kicker">{t("Keyboard")}</p><h2>{t("Global shortcuts")}</h2></div>
          </div>
          <p className="settings-note">
            {t("Shortcuts work while Watchhouse is hidden. Disabled actions remain available from the app and tray.")}
          </p>
          {([
            ["toggleFocus", "Start or end focus"],
            ["pauseFocus", "Pause or resume focus"],
            ["startTemplate", "Start first template"],
          ] as const).map(([field, label]) => (
            <label className="setting-row" key={field}>
              <div><strong>{t(label)}</strong></div>
              <select
                value={shortcuts[field] ?? ""}
                disabled={savingShortcuts}
                onChange={(event) => setShortcuts({
                  ...shortcuts,
                  [field]: event.currentTarget.value || null,
                })}
              >
                <option value="">{t("Disabled")}</option>
                {[
                  "CommandOrControl+Shift+F",
                  "CommandOrControl+Shift+P",
                  "CommandOrControl+Shift+1",
                  "CommandOrControl+Shift+2",
                  "CommandOrControl+Shift+3",
                ].map((shortcut) => (
                  <option value={shortcut} key={shortcut}>{shortcut.replace("CommandOrControl", "⌘")}</option>
                ))}
              </select>
            </label>
          ))}
          <div className="data-actions">
            <button
              type="button"
              disabled={savingShortcuts}
              onClick={() => {
                setSavingShortcuts(true);
                setMessage(null);
                void updateShortcutSettings(shortcuts)
                  .then((saved) => {
                    setShortcuts(saved);
                    setMessage("Global shortcuts saved.");
                  })
                  .catch((error) => setMessage(errorMessage(error)))
                  .finally(() => setSavingShortcuts(false));
              }}
            >
              {t("Save shortcuts")}
            </button>
          </div>
        </section>
      )}

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">{t("Data")}</p><h2>{t("Local storage")}</h2></div></div>
        <p className="settings-path">{diagnostics?.databasePath ?? t("Loading database location…")}</p>
        <div className="data-actions">
          <button onClick={() => void openDataDirectory()}>{t("Show in Finder")}</button>
          <button onClick={() => void backupDatabase().then((path) => path && setMessage(`Backup saved to ${path}`))}>{t("Back Up Database")}</button>
          <button onClick={() => void optimizeDatabase().then(() => {
            setMessage("Database optimized.");
            void getDiagnosticsSummary().then(setDiagnostics);
          })}>{t("Optimize Database")}</button>
          <button onClick={() => void exportActivity("json").then((path) => path && setMessage(`Exported to ${path}`))}>{t("Export JSON")}</button>
          <button onClick={() => void exportActivity("csv").then((path) => path && setMessage(`Exported to ${path}`))}>{t("Export CSV")}</button>
        </div>
        <button className="secondary-action" onClick={() => {
          if (window.confirm(t("Restore a Watchhouse database backup? Current activity data will be replaced and tracking will pause."))) {
            void restoreDatabase().then((restored) => {
              if (restored) {
                setMessage("Database restored. Review the data, then resume tracking.");
                notifyActivityDataChanged();
                void getDiagnosticsSummary().then(setDiagnostics);
                void getSettings().then(setSettings);
              }
            }).catch((error) => setMessage(errorMessage(error)));
          }
        }}>{t("Restore Database Backup")}</button>
        <button className="secondary-action" onClick={() => {
          void clearApplicationIconCache().then(() => {
            clearApplicationIconMemoryCache();
            setMessage("Application icons will be reloaded automatically.");
            void getDiagnosticsSummary().then(setDiagnostics);
          }).catch((error) => setMessage(errorMessage(error)));
        }}>{t("Refresh Application Icons")}</button>
        <div className="maintenance-settings">
          <div className="list-heading">
            <div><p className="section-kicker">{t("Maintenance")}</p><h2>{t("Retention and backups")}</h2></div>
          </div>
          <label className="setting-row">
            <div>
              <strong>{t("Keep Activity")}</strong>
              <small>
                {t(maintenancePreview?.expiredSessionCount
                  ? `${maintenancePreview.expiredSessionCount} old sessions are eligible for cleanup.`
                  : "No sessions currently need cleanup.")}
              </small>
            </div>
            <select
              value={settings.retentionDays}
              disabled={saving}
              onChange={(event) => void save({
                ...settings,
                retentionDays: Number(event.currentTarget.value) as SettingsModel["retentionDays"],
              })}
            >
              <option value={0}>{t("Forever")}</option>
              <option value={30}>{t("30 days")}</option>
              <option value={90}>{t("90 days")}</option>
              <option value={180}>{t("180 days")}</option>
              <option value={365}>{t("1 year")}</option>
            </select>
          </label>
          <div className="setting-row">
            <div><strong>{t("Automatic Backups")}</strong><small>{t("Create local SQLite backups on schedule.")}</small></div>
            <Toggle
              checked={settings.automaticBackupEnabled}
              disabled={saving}
              onChange={(value) => void save({ ...settings, automaticBackupEnabled: value })}
            />
          </div>
          <label className="setting-row">
            <div><strong>{t("Backup Schedule")}</strong><small>{t("Old automatic backups are rotated.")}</small></div>
            <span className="maintenance-inline">
              <select
                value={settings.backupInterval}
                disabled={saving || !settings.automaticBackupEnabled}
                onChange={(event) => void save({
                  ...settings,
                  backupInterval: event.currentTarget.value as "DAILY" | "WEEKLY",
                })}
              >
                <option value="DAILY">{t("Daily")}</option>
                <option value="WEEKLY">{t("Weekly")}</option>
              </select>
              <select
                value={settings.backupKeepCount}
                disabled={saving || !settings.automaticBackupEnabled}
                onChange={(event) => void save({
                  ...settings,
                  backupKeepCount: Number(event.currentTarget.value),
                })}
                aria-label={t("Automatic backups to keep")}
              >
                {[3, 5, 10, 20].map((count) => (
                  <option value={count} key={count}>{t(`Keep ${count}`)}</option>
                ))}
              </select>
            </span>
          </label>
          <p className="settings-path">
            {settings.backupDirectory ?? t("Default application data / backups")}
          </p>
          <div className="data-actions">
            <button onClick={() => void chooseBackupDirectory().then((directory) => {
              if (directory) void save({ ...settings, backupDirectory: directory });
            })}>{t("Choose Backup Folder")}</button>
            <button onClick={() => void openBackupDirectory()}>{t("Show Backup Folder")}</button>
            <button onClick={() => void createAutomaticBackupNow()
              .then((path) => {
                setMessage(`Backup saved to ${path}`);
                void getSettings().then(setSettings);
                void getDiagnosticsSummary().then(setDiagnostics);
              })
              .catch((error) => setMessage(errorMessage(error)))}>{t("Back Up Now")}</button>
            <button onClick={() => {
              const count = maintenancePreview?.expiredSessionCount ?? 0;
              if (window.confirm(t(
                count
                  ? `Delete ${count} expired sessions and unused application data?`
                  : "Clean unused application data and optimize retention metadata?",
              ))) {
                void runDataMaintenance().then((result) => {
                  setMessage(
                    `Maintenance complete: ${result.deletedSessionCount} sessions and ${result.deletedApplicationIds.length} unused applications removed.`,
                  );
                  notifyActivityDataChanged();
                  void getSettings().then(setSettings);
                  void getMaintenancePreview().then(setMaintenancePreview);
                  void getDiagnosticsSummary().then(setDiagnostics);
                }).catch((error) => setMessage(errorMessage(error)));
              }
            }}>{t("Clean Up Now")}</button>
          </div>
          <div className="data-health">
            <div>
              <strong>{t("Data Health")}</strong>
              <small>
                {t(dataHealth && dataHealth.overlappingSessionCount + dataHealth.zeroDurationSessionCount > 0
                  ? `${dataHealth.overlappingSessionCount} overlapping and ${dataHealth.zeroDurationSessionCount} zero-duration sessions found.`
                  : "No repairable session problems found.")}
              </small>
            </div>
            <button
              type="button"
              disabled={!dataHealth || dataHealth.overlappingSessionCount + dataHealth.zeroDurationSessionCount === 0}
              onClick={() => {
                if (!window.confirm(t("Repair overlapping and zero-duration closed sessions? Back up first if you may need the original timestamps."))) return;
                void repairDataHealth()
                  .then((result) => {
                    setMessage(`Data repaired: ${result.trimmedSessionCount} trimmed and ${result.deletedSessionCount} removed.`);
                    notifyActivityDataChanged();
                    void getDataHealthSummary().then(setDataHealth);
                    void getDataHealthUndoStatus().then(setDataHealthUndo);
                  })
                  .catch((error) => setMessage(errorMessage(error)));
              }}
            >
              {t("Repair Problems")}
            </button>
            <button
              type="button"
              disabled={!dataHealthUndo?.available}
              onClick={() => void undoDataHealthRepair()
                .then((count) => {
                  setMessage(`Restored ${count} sessions from the last health repair.`);
                  notifyActivityDataChanged();
                  void getDataHealthSummary().then(setDataHealth);
                  void getDataHealthUndoStatus().then(setDataHealthUndo);
                })
                .catch((error) => setMessage(errorMessage(error)))}
            >
              {t("Undo Last Repair")}
            </button>
          </div>
          {dataHealthUndo?.backupPath && (
            <p className="settings-path">{t("Safety backup:")} {dataHealthUndo.backupPath}</p>
          )}
          <p className="settings-note">
            {t("Last cleanup:")} {formatOptionalDate(settings.lastMaintenanceAtMs, locale, t("Never"))}
            {" · "}
            {t("Last backup:")} {formatOptionalDate(settings.lastBackupAtMs, locale, t("Never"))}
          </p>
          {maintenanceStatus?.running && (
            <p className="settings-note">{t("Automatic maintenance is running.")}</p>
          )}
          {maintenanceStatus?.lastError && (
            <p className="maintenance-error">
              {t("Automatic maintenance failed:")} {maintenanceStatus.lastError}
            </p>
          )}
        </div>
        <button className="danger-button" onClick={() => {
          if (window.confirm(t("Delete all recorded activity? This cannot be undone."))) {
            void deleteAllActivity().then(() => {
              setMessage("All activity data was deleted.");
              notifyActivityDataChanged();
              void getDiagnosticsSummary().then(setDiagnostics);
            })
              .catch((error) => setMessage(errorMessage(error)));
          }
        }}>{t("Delete All Activity Data")}</button>
      </section>

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">{t("Privacy")}</p><h2>{t("Local-first protection")}</h2></div></div>
        <p className="settings-note">{t("Review what Watchhouse records and what it deliberately avoids collecting.")}</p>
        <div className="data-actions">
          <button onClick={() => setShowPrivacy(true)}>{t("View Privacy Notice")}</button>
          <button onClick={() => void openLogDirectory()}>{t("Show Diagnostic Logs")}</button>
        </div>
      </section>

      <SoftwareUpdates />

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">{t("About")}</p><h2>{t("YunQi-Watchhouse")}</h2></div><span>{t(diagnostics ? `Version ${diagnostics.applicationVersion}` : "Version…")}</span></div>
        <p className="settings-note">{t("Private, local-first computer activity timeline.")}</p>
        {diagnostics && (
          <div className="diagnostics-grid">
            <div><span>{t("Version")}</span><strong>{diagnostics.applicationVersion}</strong></div>
            <div><span>{t("Sessions")}</span><strong>{diagnostics.sessionCount}</strong></div>
            <div><span>{t("Applications")}</span><strong>{diagnostics.applicationCount}</strong></div>
            <div><span>{t("Database")}</span><strong>{formatBytes(diagnostics.databaseBytes)}</strong></div>
            <div><span>{t("WAL")}</span><strong>{formatBytes(diagnostics.walBytes)}</strong></div>
            <div><span>{t("Icons")}</span><strong>{formatBytes(diagnostics.iconCacheBytes)}</strong></div>
            <div><span>{t("Logs")}</span><strong>{formatBytes(diagnostics.logBytes)}</strong></div>
            <div><span>{t("Backups")}</span><strong>{diagnostics.automaticBackupCount} · {formatBytes(diagnostics.automaticBackupBytes)}</strong></div>
          </div>
        )}
      </section>
      {showPrivacy && <PrivacyNotice onClose={() => setShowPrivacy(false)} />}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatOptionalDate(timestamp: number, locale: string, neverLabel: string): string {
  return timestamp > 0
    ? new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" })
      .format(timestamp)
    : neverLabel;
}
