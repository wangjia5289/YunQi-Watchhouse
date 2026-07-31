import { useEffect, useRef, useState } from "react";
import {
  DiagnosticsSummary,
  Settings as SettingsModel,
  backupDatabase,
  clearApplicationIconCache,
  deleteAllActivity,
  exportActivity,
  errorMessage,
  getSettings,
  getDiagnosticsSummary,
  openDataDirectory,
  openLogDirectory,
  optimizeDatabase,
  restoreDatabase,
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

  useEffect(() => {
    void getSettings()
      .then(setSettings)
      .catch((error) => setMessage(errorMessage(error)));
    void getDiagnosticsSummary().then(setDiagnostics);
  }, []);

  useEffect(() => {
    if (settings) {
      document.documentElement.dataset.theme = settings.appearance.toLowerCase();
    }
  }, [settings?.appearance]);

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
