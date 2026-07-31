import { useEffect, useState } from "react";
import "./App.css";
import { Dashboard } from "./features/dashboard/Dashboard";
import { Timeline } from "./features/timeline/Timeline";
import { Applications } from "./features/applications/Applications";
import { History } from "./features/history/History";
import { Settings } from "./features/settings/Settings";
import { Reports } from "./features/reports/Reports";
import {
  CurrentActivity,
  errorMessage,
  getCurrentActivity,
  getSettings,
} from "./lib/ipc";
import { completeOnboarding, setTrackingPaused } from "./lib/ipc";
import { PrivacyNotice } from "./features/onboarding/PrivacyNotice";
import { listen } from "@tauri-apps/api/event";
import { notifyActivityDataChanged } from "./lib/events";

const navigation = [
  { label: "Today", icon: "today", page: "today", enabled: true },
  { label: "Timeline", icon: "timeline", page: "timeline", enabled: true },
  { label: "Applications", icon: "apps", page: "applications", enabled: true },
  { label: "History", icon: "history", page: "history", enabled: true },
  { label: "Reports", icon: "reports", page: "reports", enabled: true },
] as const;

type Page = (typeof navigation)[number]["page"] | "settings";

function NavIcon({ name }: { name: string }) {
  const paths: Record<string, React.ReactNode> = {
    today: <path d="M5 4.5h14v15H5zM8 2.5v4M16 2.5v4M5 9h14" />,
    timeline: <path d="M6 4v16M6 7h5M6 12h9M6 17h7" />,
    apps: (
      <>
        <rect x="4" y="4" width="6" height="6" rx="1.5" />
        <rect x="14" y="4" width="6" height="6" rx="1.5" />
        <rect x="4" y="14" width="6" height="6" rx="1.5" />
        <rect x="14" y="14" width="6" height="6" rx="1.5" />
      </>
    ),
    history: <path d="M4 12a8 8 0 1 0 2.3-5.7L4 8.6M4 4v4.6h4.6M12 7.5V12l3 2" />,
    reports: <path d="M5 19V9M12 19V5M19 19v-7M3 19h18" />,
    settings: (
      <>
        <circle cx="12" cy="12" r="3" />
        <path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6 7 7M17 17l1.4 1.4M18.4 5.6 17 7M7 17l-1.4 1.4" />
      </>
    ),
  };

  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      {paths[name]}
    </svg>
  );
}

function App() {
  const [page, setPage] = useState<Page>("today");
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  const [needsOnboarding, setNeedsOnboarding] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [tracking, setTracking] = useState<CurrentActivity | null>(null);

  function loadSettings() {
    setSettingsError(null);
    void getSettings().then((settings) => {
      document.documentElement.dataset.theme = settings.appearance.toLowerCase();
      setNeedsOnboarding(!settings.onboardingCompleted);
      setSettingsLoaded(true);
    }).catch((error) => {
      setSettingsLoaded(false);
      setSettingsError(errorMessage(error));
    });
  }

  useEffect(() => {
    loadSettings();
    const loadTracking = () => void getCurrentActivity().then(setTracking).catch(() => {});
    loadTracking();
    const timer = window.setInterval(loadTracking, 2_000);
    const refreshVisibleData = () => {
      if (document.visibilityState === "visible") notifyActivityDataChanged();
    };
    window.addEventListener("focus", refreshVisibleData);
    document.addEventListener("visibilitychange", refreshVisibleData);
    const unlisten = listen("activity-data-changed", notifyActivityDataChanged);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", refreshVisibleData);
      document.removeEventListener("visibilitychange", refreshVisibleData);
      void unlisten.then((stop) => stop());
    };
  }, []);

  return (
    <>
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <span />
          </span>
          <span>Watchhouse</span>
        </div>

        <nav aria-label="Main navigation">
          <p className="nav-label">Activity</p>
          {navigation.map((item) => (
            <button
              className={`nav-item${page === item.page ? " active" : ""}`}
              type="button"
              key={item.label}
              aria-current={page === item.page ? "page" : undefined}
              disabled={!item.enabled}
              onClick={() => setPage(item.page)}
            >
              <NavIcon name={item.icon} />
              {item.label}
            </button>
          ))}
        </nav>

        <div className="sidebar-footer">
          <button
            className={`global-tracking${tracking?.paused ? " paused" : ""}`}
            type="button"
            disabled={!tracking}
            onClick={() => {
              const paused = !tracking?.paused;
              void setTrackingPaused(paused)
                .then(() => getCurrentActivity())
                .then(setTracking);
            }}
          >
            <span aria-hidden="true" />
            <span>
              <strong>{!tracking ? "Checking status" : tracking.paused ? "Tracking paused" : "Tracking active"}</strong>
              <small>{!tracking ? "Connecting…" : tracking.paused ? "Click to resume" : "Click to pause"}</small>
            </span>
          </button>
          <button className={`nav-item settings-link${page === "settings" ? " active" : ""}`}
            type="button" onClick={() => setPage("settings")}>
            <NavIcon name="settings" />
            Settings
          </button>
        </div>
      </aside>

      <main className="main-content">
        {settingsError && (
          <div className="error-banner" role="alert">
            <span>{settingsError}</span>
            <button type="button" onClick={loadSettings}>Retry</button>
          </div>
        )}
        {page === "today" && <Dashboard />}
        {page === "timeline" && <Timeline />}
        {page === "applications" && <Applications />}
        {page === "history" && <History />}
        {page === "reports" && <Reports />}
        {page === "settings" && <Settings />}
      </main>
    </div>
    {settingsLoaded && needsOnboarding && (
      <PrivacyNotice onboarding onAccept={() => {
        void completeOnboarding().then((settings) => {
          setNeedsOnboarding(false);
          if (settings.startTrackingAutomatically) void setTrackingPaused(false);
        });
      }} />
    )}
    </>
  );
}

export default App;
