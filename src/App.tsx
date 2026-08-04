import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import "./App.css";
import "./styles/tokens.css";
import {
  CurrentActivity,
  errorMessage,
  getCurrentActivity,
  getSettings,
} from "./lib/ipc";
import { completeOnboarding, setTrackingPaused } from "./lib/ipc";
import { PrivacyNotice } from "./features/onboarding/PrivacyNotice";
import { PageErrorBoundary } from "./components/PageErrorBoundary";
import { emit, listen } from "@tauri-apps/api/event";
import { notifyActivityDataChanged } from "./lib/events";
import { useLocale } from "./lib/i18n";

const Dashboard = lazy(() => import("./features/dashboard/Dashboard")
  .then((module) => ({ default: module.Dashboard })));
const Timeline = lazy(() => import("./features/timeline/Timeline")
  .then((module) => ({ default: module.Timeline })));
const GlobalSearch = lazy(() => import("./features/search/GlobalSearch")
  .then((module) => ({ default: module.GlobalSearch })));
const Applications = lazy(() => import("./features/applications/Applications")
  .then((module) => ({ default: module.Applications })));
const History = lazy(() => import("./features/history/History")
  .then((module) => ({ default: module.History })));
const Reports = lazy(() => import("./features/reports/Reports")
  .then((module) => ({ default: module.Reports })));
const Settings = lazy(() => import("./features/settings/Settings")
  .then((module) => ({ default: module.Settings })));

const navigation = [
  { label: "Today", icon: "today", page: "today", enabled: true },
  { label: "Timeline", icon: "timeline", page: "timeline", enabled: true },
  { label: "Search", icon: "search", page: "search", enabled: true },
  { label: "Applications", icon: "apps", page: "applications", enabled: true },
  { label: "History", icon: "history", page: "history", enabled: true },
  { label: "Reports", icon: "reports", page: "reports", enabled: true },
] as const;

type Page = (typeof navigation)[number]["page"] | "settings";

function NavIcon({ name }: { name: string }) {
  const paths: Record<string, React.ReactNode> = {
    today: <path d="M5 4.5h14v15H5zM8 2.5v4M16 2.5v4M5 9h14" />,
    timeline: <path d="M6 4v16M6 7h5M6 12h9M6 17h7" />,
    search: (
      <>
        <circle cx="10.5" cy="10.5" r="6.5" />
        <path d="m15.5 15.5 4 4" />
      </>
    ),
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

function PageLoading() {
  const { t } = useLocale();
  return (
    <div className="page-loading" role="status">
      <span className="page-loading-indicator" aria-hidden="true" />
      <span>{t("Loading…")}</span>
    </div>
  );
}

function MainApp() {
  const { locale, setLocale, t } = useLocale();
  const [page, setPage] = useState<Page>("today");
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  const [needsOnboarding, setNeedsOnboarding] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [tracking, setTracking] = useState<CurrentActivity | null>(null);
  const [timelineTarget, setTimelineTarget] = useState<{
    date?: string;
    sessionId?: number;
  }>({});

  const loadTracking = useCallback(async () => {
    try {
      setTracking(await getCurrentActivity());
    } catch {
      // A later poll or activity event will retry transient status failures.
    }
  }, []);

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
    let trackingTimer: number | null = null;
    let windowFocused = document.hasFocus();

    const isWindowActive = () =>
      document.visibilityState === "visible" && windowFocused;

    const stopTrackingPolling = () => {
      if (trackingTimer === null) return;
      window.clearInterval(trackingTimer);
      trackingTimer = null;
    };
    const startTrackingPolling = () => {
      if (!isWindowActive() || trackingTimer !== null) return;
      void loadTracking();
      trackingTimer = window.setInterval(() => {
        if (!isWindowActive()) {
          stopTrackingPolling();
          return;
        }
        void loadTracking();
      }, 2_000);
    };
    const refreshVisibleData = () => {
      if (!isWindowActive()) return;
      startTrackingPolling();
      notifyActivityDataChanged();
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") refreshVisibleData();
      else stopTrackingPolling();
    };
    const handleActivityDataChanged = () => {
      if (!isWindowActive()) return;
      void loadTracking();
      notifyActivityDataChanged();
    };
    const handleWindowFocus = () => {
      windowFocused = true;
      refreshVisibleData();
    };
    const handleWindowBlur = () => {
      windowFocused = false;
      stopTrackingPolling();
    };

    startTrackingPolling();
    window.addEventListener("focus", handleWindowFocus);
    window.addEventListener("blur", handleWindowBlur);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    const unlisten = listen("activity-data-changed", handleActivityDataChanged);
    return () => {
      stopTrackingPolling();
      window.removeEventListener("focus", handleWindowFocus);
      window.removeEventListener("blur", handleWindowBlur);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      void unlisten.then((stop) => stop());
    };
  }, [loadTracking]);

  useEffect(() => {
    void emit("locale-changed", locale).catch(() => {
      // Browser preview does not expose Tauri events.
    });
  }, [locale]);

  return (
    <>
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <span />
          </span>
          <span>{t("Watchhouse")}</span>
        </div>

        <nav aria-label={t("Main navigation")}>
          <p className="nav-label">{t("Activity")}</p>
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
              {t(item.label)}
            </button>
          ))}
        </nav>

        <div className="sidebar-footer">
          <div className="language-switch" role="group" aria-label={t("Interface language")}>
            <button
              type="button"
              className={locale === "zh-CN" ? "active" : ""}
              aria-pressed={locale === "zh-CN"}
              onClick={() => setLocale("zh-CN")}
            >
              中文
            </button>
            <button
              type="button"
              className={locale === "en" ? "active" : ""}
              aria-pressed={locale === "en"}
              onClick={() => setLocale("en")}
            >
              EN
            </button>
          </div>
          <button
            className={`global-tracking${tracking?.paused ? " paused" : ""}`}
            type="button"
            aria-label={t(!tracking ? "Checking status" : tracking.paused ? "Tracking paused" : "Tracking active")}
            disabled={!tracking}
            onClick={() => {
              const paused = !tracking?.paused;
              void setTrackingPaused(paused)
                .then(loadTracking);
            }}
          >
            <span aria-hidden="true" />
            <span>
              <strong>{t(!tracking ? "Checking status" : tracking.paused ? "Tracking paused" : "Tracking active")}</strong>
              <small>{t(!tracking ? "Connecting…" : tracking.paused ? "Click to resume" : "Click to pause")}</small>
            </span>
          </button>
          <button className={`nav-item settings-link${page === "settings" ? " active" : ""}`}
            type="button" onClick={() => setPage("settings")}>
            <NavIcon name="settings" />
            {t("Settings")}
          </button>
        </div>
      </aside>

      <main className="main-content">
        {settingsError && (
          <div className="error-banner" role="alert">
            <span>{t(settingsError)}</span>
            <button type="button" onClick={loadSettings}>{t("Retry")}</button>
          </div>
        )}
        <PageErrorBoundary resetKey={page}>
          <Suspense fallback={<PageLoading />}>
            {page === "today" && (
              <Dashboard current={tracking} onTrackingChanged={loadTracking} />
            )}
            {page === "timeline" && (
              <Timeline
                initialDate={timelineTarget.date}
                initialSessionId={timelineTarget.sessionId}
                onDateChange={(date) => setTimelineTarget({ date })}
              />
            )}
            {page === "search" && (
              <GlobalSearch
                onOpenDate={(date, sessionId) => {
                  setTimelineTarget({ date, sessionId });
                  setPage("timeline");
                }}
              />
            )}
            {page === "applications" && <Applications />}
            {page === "history" && <History />}
            {page === "reports" && <Reports />}
            {page === "settings" && <Settings />}
          </Suspense>
        </PageErrorBoundary>
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

export default MainApp;
