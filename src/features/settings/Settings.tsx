import { useEffect, useRef, useState } from "react";
import {
  DiagnosticsSummary,
  MaintenancePreview,
  Settings as SettingsModel,
  backupDatabase,
  chooseBackupDirectory,
  clearApplicationIconCache,
  createAutomaticBackupNow,
  deleteAllActivity,
  exportActivity,
  errorMessage,
  getSettings,
  getDiagnosticsSummary,
  getMaintenancePreview,
  openDataDirectory,
  openBackupDirectory,
  openLogDirectory,
  optimizeDatabase,
  restoreDatabase,
  runDataMaintenance,
  updateSettings,
} from "../../lib/ipc";
import { notifyActivityDataChanged } from "../../lib/events";
import { clearApplicationIconMemoryCache } from "../applications/ApplicationIcon";
import { PrivacyNotice } from "../onboarding/PrivacyNotice";

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
  const [settings, setSettings] = useState<SettingsModel | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [showPrivacy, setShowPrivacy] = useState(false);
  const [diagnostics, setDiagnostics] = useState<DiagnosticsSummary | null>(null);
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const [maintenancePreview, setMaintenancePreview] = useState<MaintenancePreview | null>(null);

  useEffect(() => {
    void getSettings()
      .then(setSettings)
      .catch((error) => setMessage(errorMessage(error)));
    void getDiagnosticsSummary().then(setDiagnostics);
    void getMaintenancePreview().then(setMaintenancePreview);
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
    return <div className="settings-page"><p>Loading settings…</p></div>;
  }

  return (
    <div className="settings-page">
      <header><div><p className="date-label">Preferences</p><h1>Settings</h1></div></header>
      {message && <div className="settings-message">{message}</div>}

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">General</p><h2>Application</h2></div></div>
        <div className="setting-row">
          <div><strong>Launch at Login</strong><small>Start Watchhouse when you sign in.</small></div>
          <Toggle checked={settings.launchAtLogin} disabled={saving}
            onChange={(value) => void save({ ...settings, launchAtLogin: value })} />
        </div>
        <div className="setting-row">
          <div><strong>Hide to Tray on Close</strong><small>Keep tracking when the window closes.</small></div>
          <Toggle checked={settings.hideToTrayOnClose} disabled={saving}
            onChange={(value) => void save({ ...settings, hideToTrayOnClose: value })} />
        </div>
        <div className="setting-row">
          <div><strong>Start Tracking Automatically</strong><small>Begin recording when Watchhouse starts.</small></div>
          <Toggle checked={settings.startTrackingAutomatically} disabled={saving}
            onChange={(value) => void save({ ...settings, startTrackingAutomatically: value })} />
        </div>
      </section>

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">Tracking</p><h2>Activity detection</h2></div></div>
        <label className="setting-row">
          <div><strong>Idle Threshold</strong><small>Changes apply immediately.</small></div>
          <select value={settings.idleThresholdSeconds} disabled={saving}
            onChange={(event) => void save({ ...settings, idleThresholdSeconds: Number(event.currentTarget.value) })}>
            <option value={60}>1 minute</option><option value={180}>3 minutes</option>
            <option value={300}>5 minutes</option><option value={600}>10 minutes</option>
            <option value={900}>15 minutes</option>
          </select>
        </label>
        <div className="setting-row disabled">
          <div><strong>Record Window Titles</strong><small>Requires a future Accessibility permission module.</small></div>
          <Toggle checked={false} disabled onChange={() => {}} />
        </div>
      </section>

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">Appearance</p><h2>Theme</h2></div></div>
        <div className="settings-segmented">
          {(["SYSTEM", "LIGHT", "DARK"] as const).map((value) => (
            <button key={value} disabled={saving}
              className={settings.appearance === value ? "active" : ""}
              onClick={() => void save({ ...settings, appearance: value })}>
              {value[0] + value.slice(1).toLowerCase()}
            </button>
          ))}
        </div>
      </section>

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">Data</p><h2>Local storage</h2></div></div>
        <p className="settings-path">{diagnostics?.databasePath ?? "Loading database location…"}</p>
        <div className="data-actions">
          <button onClick={() => void openDataDirectory()}>Show in Finder</button>
          <button onClick={() => void backupDatabase().then((path) => path && setMessage(`Backup saved to ${path}`))}>Back Up Database</button>
          <button onClick={() => void optimizeDatabase().then(() => {
            setMessage("Database optimized.");
            void getDiagnosticsSummary().then(setDiagnostics);
          })}>Optimize Database</button>
          <button onClick={() => void exportActivity("json").then((path) => path && setMessage(`Exported to ${path}`))}>Export JSON</button>
          <button onClick={() => void exportActivity("csv").then((path) => path && setMessage(`Exported to ${path}`))}>Export CSV</button>
        </div>
        <button className="secondary-action" onClick={() => {
          if (window.confirm("Restore a Watchhouse database backup? Current activity data will be replaced and tracking will pause.")) {
            void restoreDatabase().then((restored) => {
              if (restored) {
                setMessage("Database restored. Review the data, then resume tracking.");
                notifyActivityDataChanged();
                void getDiagnosticsSummary().then(setDiagnostics);
                void getSettings().then(setSettings);
              }
            }).catch((error) => setMessage(errorMessage(error)));
          }
        }}>Restore Database Backup</button>
        <button className="secondary-action" onClick={() => {
          void clearApplicationIconCache().then(() => {
            clearApplicationIconMemoryCache();
            setMessage("Application icons will be reloaded automatically.");
            void getDiagnosticsSummary().then(setDiagnostics);
          }).catch((error) => setMessage(errorMessage(error)));
        }}>Refresh Application Icons</button>
        <div className="maintenance-settings">
          <div className="list-heading">
            <div><p className="section-kicker">Maintenance</p><h2>Retention and backups</h2></div>
          </div>
          <label className="setting-row">
            <div>
              <strong>Keep Activity</strong>
              <small>
                {maintenancePreview?.expiredSessionCount
                  ? `${maintenancePreview.expiredSessionCount} old sessions are eligible for cleanup.`
                  : "No sessions currently need cleanup."}
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
              <option value={0}>Forever</option>
              <option value={30}>30 days</option>
              <option value={90}>90 days</option>
              <option value={180}>180 days</option>
              <option value={365}>1 year</option>
            </select>
          </label>
          <div className="setting-row">
            <div><strong>Automatic Backups</strong><small>Create local SQLite backups on schedule.</small></div>
            <Toggle
              checked={settings.automaticBackupEnabled}
              disabled={saving}
              onChange={(value) => void save({ ...settings, automaticBackupEnabled: value })}
            />
          </div>
          <label className="setting-row">
            <div><strong>Backup Schedule</strong><small>Old automatic backups are rotated.</small></div>
            <span className="maintenance-inline">
              <select
                value={settings.backupInterval}
                disabled={saving || !settings.automaticBackupEnabled}
                onChange={(event) => void save({
                  ...settings,
                  backupInterval: event.currentTarget.value as "DAILY" | "WEEKLY",
                })}
              >
                <option value="DAILY">Daily</option>
                <option value="WEEKLY">Weekly</option>
              </select>
              <select
                value={settings.backupKeepCount}
                disabled={saving || !settings.automaticBackupEnabled}
                onChange={(event) => void save({
                  ...settings,
                  backupKeepCount: Number(event.currentTarget.value),
                })}
                aria-label="Automatic backups to keep"
              >
                {[3, 5, 10, 20].map((count) => (
                  <option value={count} key={count}>Keep {count}</option>
                ))}
              </select>
            </span>
          </label>
          <p className="settings-path">
            {settings.backupDirectory ?? "Default application data / backups"}
          </p>
          <div className="data-actions">
            <button onClick={() => void chooseBackupDirectory().then((directory) => {
              if (directory) void save({ ...settings, backupDirectory: directory });
            })}>Choose Backup Folder</button>
            <button onClick={() => void openBackupDirectory()}>Show Backup Folder</button>
            <button onClick={() => void createAutomaticBackupNow()
              .then((path) => {
                setMessage(`Backup saved to ${path}`);
                void getSettings().then(setSettings);
                void getDiagnosticsSummary().then(setDiagnostics);
              })
              .catch((error) => setMessage(errorMessage(error)))}>Back Up Now</button>
            <button onClick={() => {
              const count = maintenancePreview?.expiredSessionCount ?? 0;
              if (window.confirm(
                count
                  ? `Delete ${count} expired sessions and unused application data?`
                  : "Clean unused application data and optimize retention metadata?",
              )) {
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
            }}>Clean Up Now</button>
          </div>
          <p className="settings-note">
            Last cleanup: {formatOptionalDate(settings.lastMaintenanceAtMs)} · Last backup: {formatOptionalDate(settings.lastBackupAtMs)}
          </p>
        </div>
        <button className="danger-button" onClick={() => {
          if (window.confirm("Delete all recorded activity? This cannot be undone.")) {
            void deleteAllActivity().then(() => {
              setMessage("All activity data was deleted.");
              notifyActivityDataChanged();
              void getDiagnosticsSummary().then(setDiagnostics);
            })
              .catch((error) => setMessage(errorMessage(error)));
          }
        }}>Delete All Activity Data</button>
      </section>

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">Privacy</p><h2>Local-first protection</h2></div></div>
        <p className="settings-note">Review what Watchhouse records and what it deliberately avoids collecting.</p>
        <div className="data-actions">
          <button onClick={() => setShowPrivacy(true)}>View Privacy Notice</button>
          <button onClick={() => void openLogDirectory()}>Show Diagnostic Logs</button>
        </div>
      </section>

      <section className="settings-card">
        <div className="list-heading"><div><p className="section-kicker">About</p><h2>YunQi-Watchhouse</h2></div><span>{diagnostics ? `Version ${diagnostics.applicationVersion}` : "Version…"}</span></div>
        <p className="settings-note">Private, local-first computer activity timeline.</p>
        {diagnostics && (
          <div className="diagnostics-grid">
            <div><span>Version</span><strong>{diagnostics.applicationVersion}</strong></div>
            <div><span>Sessions</span><strong>{diagnostics.sessionCount}</strong></div>
            <div><span>Applications</span><strong>{diagnostics.applicationCount}</strong></div>
            <div><span>Database</span><strong>{formatBytes(diagnostics.databaseBytes)}</strong></div>
            <div><span>WAL</span><strong>{formatBytes(diagnostics.walBytes)}</strong></div>
            <div><span>Icons</span><strong>{formatBytes(diagnostics.iconCacheBytes)}</strong></div>
            <div><span>Logs</span><strong>{formatBytes(diagnostics.logBytes)}</strong></div>
            <div><span>Backups</span><strong>{diagnostics.automaticBackupCount} · {formatBytes(diagnostics.automaticBackupBytes)}</strong></div>
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

function formatOptionalDate(timestamp: number): string {
  return timestamp > 0
    ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" })
      .format(timestamp)
    : "Never";
}
