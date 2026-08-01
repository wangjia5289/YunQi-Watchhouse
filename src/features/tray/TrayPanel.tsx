import { CSSProperties, useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, Window } from "@tauri-apps/api/window";
import { formatDuration } from "../../lib/format";
import { Locale, useLocale } from "../../lib/i18n";
import {
  CurrentActivity,
  FocusModeStatus,
  TodaySummary,
  UsageLimitProgress,
  endFocusPlan,
  errorMessage,
  getCurrentActivity,
  getFocusMode,
  getSettings,
  getTodayFocusSummary,
  getTodaySummary,
  getTodayUsageLimitProgress,
  setFocusMode,
  setTrackingPaused,
} from "../../lib/ipc";
import {
  closestUsageLimit,
  currentApplicationName,
  focusElapsedMs,
  focusRemainingMs,
  usageLimitName,
} from "./trayModel";
import "./TrayPanel.css";

interface TraySnapshot {
  current: CurrentActivity | null;
  summary: TodaySummary | null;
  focusDurationMs: number;
  focusMode: FocusModeStatus | null;
  usageLimits: UsageLimitProgress[];
}

const emptySnapshot: TraySnapshot = {
  current: null,
  summary: null,
  focusDurationMs: 0,
  focusMode: null,
  usageLimits: [],
};

const APP_NAME = "Watchhouse";

const copy = {
  en: {
    heading: "Today at a glance",
    active: "Active today",
    focus: "Focused today",
    current: "Current application",
    currentPaused: "Tracking is paused",
    idle: "No active application",
    limit: "Closest limit",
    noLimit: "No limit configured",
    trackingPause: "Pause tracking",
    trackingResume: "Resume tracking",
    focusStart: "Start focus",
    focusEnd: "End focus",
    focusActive: "Focus active",
    focusPaused: "Focus paused",
    remaining: "remaining",
    elapsed: "elapsed",
    open: "Open Watchhouse",
    loading: "Loading local activity…",
    retry: "Retry",
    loadError: "Could not load local activity.",
    actionError: "The action could not be completed.",
  },
  "zh-CN": {
    heading: "今日概览",
    active: "今日活跃",
    focus: "今日专注",
    current: "当前应用",
    currentPaused: "追踪已暂停",
    idle: "暂无活跃应用",
    limit: "最接近限额",
    noLimit: "尚未设置限额",
    trackingPause: "暂停追踪",
    trackingResume: "继续追踪",
    focusStart: "开始专注",
    focusEnd: "结束专注",
    focusActive: "专注进行中",
    focusPaused: "专注已暂停",
    remaining: "剩余",
    elapsed: "已进行",
    open: "打开 Watchhouse",
    loading: "正在读取本地活动…",
    retry: "重试",
    loadError: "无法读取本地活动。",
    actionError: "操作未能完成。",
  },
} as const;

export function TrayPanel() {
  const { locale, setLocale } = useLocale();
  const labels = copy[locale];
  const [snapshot, setSnapshot] = useState<TraySnapshot>(emptySnapshot);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"tracking" | "focus" | null>(null);
  const [nowMs, setNowMs] = useState(Date.now());

  const refresh = useCallback(async () => {
    try {
      const [current, summary, focus, focusMode, usageLimits, settings] = await Promise.all([
        getCurrentActivity(),
        getTodaySummary(),
        getTodayFocusSummary(),
        getFocusMode(),
        getTodayUsageLimitProgress(),
        getSettings(),
      ]);
      document.documentElement.dataset.theme = settings.appearance.toLowerCase();
      setSnapshot({
        current,
        summary,
        focusDurationMs: focus.totalFocusDurationMs,
        focusMode,
        usageLimits,
      });
      setError(null);
    } catch (reason) {
      setError(locale === "en" ? errorMessage(reason) : labels.loadError);
    } finally {
      setLoading(false);
    }
  }, [labels.loadError, locale]);

  useEffect(() => {
    void refresh();
    let dataTimer: number | null = null;
    let clockTimer: number | null = null;
    const stopTimers = () => {
      if (dataTimer !== null) window.clearInterval(dataTimer);
      if (clockTimer !== null) window.clearInterval(clockTimer);
      dataTimer = null;
      clockTimer = null;
    };
    const startTimers = () => {
      stopTimers();
      setNowMs(Date.now());
      void refresh();
      dataTimer = window.setInterval(() => void refresh(), 2_000);
      clockTimer = window.setInterval(() => setNowMs(Date.now()), 1_000);
    };
    const unlistenWindowFocus = getCurrentWindow().onFocusChanged((event) => {
      if (event.payload) startTimers();
      else stopTimers();
    });
    const unlistenLocale = listen<Locale>("locale-changed", (event) => {
      if (event.payload === "en" || event.payload === "zh-CN") setLocale(event.payload);
    });
    const unlistenActivity = listen("activity-data-changed", () => void refresh());
    const unlistenFocus = listen<FocusModeStatus>("focus-mode-changed", (event) => {
      setSnapshot((current) => ({ ...current, focusMode: event.payload }));
      void refresh();
    });
    const unlistenShown = listen("tray-panel-shown", () => void refresh());
    return () => {
      stopTimers();
      void unlistenWindowFocus.then((stop) => stop());
      void unlistenLocale.then((stop) => stop());
      void unlistenActivity.then((stop) => stop());
      void unlistenFocus.then((stop) => stop());
      void unlistenShown.then((stop) => stop());
    };
  }, [refresh, setLocale]);

  const currentName = currentApplicationName(snapshot.current);
  const closestLimit = useMemo(
    () => closestUsageLimit(snapshot.usageLimits),
    [snapshot.usageLimits],
  );
  const focusModeTime = focusRemainingMs(snapshot.focusMode, nowMs);
  const focusModeDuration = focusModeTime ?? focusElapsedMs(snapshot.focusMode, nowMs);

  async function toggleTracking() {
    if (!snapshot.current || busy) return;
    setBusy("tracking");
    try {
      await setTrackingPaused(!snapshot.current.paused);
      await refresh();
    } catch (reason) {
      setError(locale === "en" ? errorMessage(reason) : labels.actionError);
    } finally {
      setBusy(null);
    }
  }

  async function toggleFocus() {
    if (busy) return;
    setBusy("focus");
    try {
      const focusMode = snapshot.focusMode?.active
        ? await endFocusPlan(false)
        : await setFocusMode(true);
      setSnapshot((current) => ({ ...current, focusMode }));
      await refresh();
    } catch (reason) {
      setError(locale === "en" ? errorMessage(reason) : labels.actionError);
    } finally {
      setBusy(null);
    }
  }

  async function openMainWindow() {
    try {
      const main = await Window.getByLabel("main");
      if (main) {
        await main.show();
        await main.unminimize();
        await main.setFocus();
      }
      await getCurrentWindow().hide();
    } catch (reason) {
      setError(locale === "en" ? errorMessage(reason) : labels.actionError);
    }
  }

  return (
    <main className="tray-panel">
      <header className="tray-panel-header">
        <span className="tray-panel-mark" aria-hidden="true"><i /></span>
        <div>
          <strong>{APP_NAME}</strong>
          <span>{labels.heading}</span>
        </div>
        <span className={`tray-panel-live${snapshot.current?.paused ? " paused" : ""}`} aria-hidden="true" />
      </header>

      {error && (
        <div className="tray-panel-error" role="alert">
          <span>{error}</span>
          <button type="button" onClick={() => void refresh()}>{labels.retry}</button>
        </div>
      )}

      <section className="tray-panel-current" aria-label={labels.current}>
        <span>{labels.current}</span>
        <strong title={currentName ?? undefined}>
          {loading
            ? labels.loading
            : snapshot.current?.paused
              ? labels.currentPaused
              : currentName ?? labels.idle}
        </strong>
        {snapshot.focusMode?.active && (
          <small>
            {snapshot.focusMode.paused ? labels.focusPaused : labels.focusActive}
            {" · "}{focusModeTime === null ? labels.elapsed : labels.remaining}{" "}
            {formatDuration(focusModeDuration, locale)}
          </small>
        )}
      </section>

      <section className="tray-panel-stats">
        <article>
          <span>{labels.active}</span>
          <strong>{loading ? "—" : formatDuration(snapshot.summary?.activeDurationMs ?? 0, locale)}</strong>
        </article>
        <article>
          <span>{labels.focus}</span>
          <strong>{loading ? "—" : formatDuration(snapshot.focusDurationMs, locale)}</strong>
        </article>
      </section>

      <section className="tray-panel-limit" aria-label={labels.limit}>
        <div>
          <span>{labels.limit}</span>
          <strong title={closestLimit ? usageLimitName(closestLimit) : undefined}>
            {closestLimit ? usageLimitName(closestLimit) : labels.noLimit}
          </strong>
        </div>
        {closestLimit && (
          <>
            <b>{Math.max(0, Math.round(closestLimit.percentage))}%</b>
            <span className="tray-panel-limit-track" aria-hidden="true">
              <i style={{ "--tray-limit": `${Math.min(100, Math.max(0, closestLimit.percentage))}%` } as CSSProperties} />
            </span>
          </>
        )}
      </section>

      <div className="tray-panel-actions">
        <button
          type="button"
          disabled={!snapshot.current || busy !== null}
          onClick={() => void toggleTracking()}
        >
          <span className={snapshot.current?.paused ? "play" : "pause"} aria-hidden="true" />
          {snapshot.current?.paused ? labels.trackingResume : labels.trackingPause}
        </button>
        <button
          type="button"
          className={snapshot.focusMode?.active ? "danger" : "primary"}
          disabled={busy !== null}
          onClick={() => void toggleFocus()}
        >
          <span className="focus" aria-hidden="true" />
          {snapshot.focusMode?.active ? labels.focusEnd : labels.focusStart}
        </button>
      </div>

      <button className="tray-panel-open" type="button" onClick={() => void openMainWindow()}>
        {labels.open}<span aria-hidden="true">›</span>
      </button>
    </main>
  );
}
