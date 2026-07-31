import { CSSProperties } from "react";
import { formatClock, formatDuration, formatLongDate } from "../../lib/format";
import { TimelineEntry, setTrackingPaused } from "../../lib/ipc";
import { useDashboard } from "./useDashboard";

function ActivityDistribution({
  timeline,
  startMs,
  endMs,
}: {
  timeline: TimelineEntry[];
  startMs: number;
  endMs: number;
}) {
  const dayDuration = endMs - startMs;
  const segments = timeline
    .filter((entry) => entry.state === "ACTIVE" && entry.durationMs > 0)
    .map((entry) => {
      const left = ((entry.startedAtMs - startMs) / dayDuration) * 100;
      const width = (entry.durationMs / dayDuration) * 100;
      return {
        entry,
        style: {
          "--segment-left": `${Math.max(0, left)}%`,
          "--segment-width": `${Math.max(0.18, width)}%`,
        } as CSSProperties,
      };
    });

  return (
    <section className="activity-card" aria-labelledby="activity-heading">
      <div className="section-heading">
        <div>
          <p className="section-kicker">Overview</p>
          <h2 id="activity-heading">Today&apos;s activity</h2>
        </div>
        <span className="session-count">
          {timeline.filter((entry) => entry.state === "ACTIVE").length} sessions
        </span>
      </div>

      <div className="time-scale" aria-hidden="true">
        {["12a", "3a", "6a", "9a", "12p", "3p", "6p", "9p", "12a"].map(
          (label, index) => (
            <span key={`${label}-${index}`}>{label}</span>
          ),
        )}
      </div>
      <div
        className="activity-track"
        role="img"
        aria-label={`${segments.length} active periods recorded today`}
      >
        <div className="track-grid" aria-hidden="true" />
        {segments.map(({ entry, style }) => (
          <span
            className="activity-segment"
            style={style}
            key={entry.sessionId}
            title={`${entry.applicationName ?? "Active"} · ${formatDuration(entry.durationMs)}`}
          />
        ))}
      </div>
      <div className="activity-legend">
        <span>
          <i className="legend-dot active" /> Active
        </span>
        <span>
          <i className="legend-dot quiet" /> No activity recorded
        </span>
      </div>
    </section>
  );
}

export function Dashboard() {
  const { summary, timeline, current, loading, error, refresh } = useDashboard();
  const runningSample =
    current?.monitor.status === "RUNNING" ? current.monitor.payload : null;
  const degraded =
    current?.monitor.status === "DEGRADED" ||
    current?.persistence.status === "DEGRADED";
  const trackingLabel = degraded
    ? "Needs attention"
    : current?.paused
      ? "Tracking paused"
      : runningSample?.state === "IDLE"
      ? "Idle"
      : "Tracking";
  const currentApp =
    current?.paused
      ? "No new activity is being recorded"
      : runningSample?.state === "ACTIVE"
      ? runningSample.foregroundApplication?.name
      : runningSample?.state === "IDLE"
        ? "Away from computer"
        : "Starting activity monitor";

  return (
    <div className="dashboard">
      <header className="topbar">
        <div>
          <p className="date-label">{formatLongDate(new Date())}</p>
          <h1>Today</h1>
        </div>
        <button
          type="button"
          className={`tracking-pill${degraded ? " degraded" : ""}${current?.paused ? " paused" : ""}`}
          onClick={() => void setTrackingPaused(!current?.paused).then(refresh)}
          title={current?.paused ? "Resume tracking" : "Pause tracking"}
        >
          <span className="tracking-dot" />
          <span>
            <strong>{trackingLabel}</strong>
            <small>{currentApp}</small>
          </span>
        </button>
      </header>

      {error && (
        <div className="error-banner" role="alert">
          <span>{error}</span>
          <button type="button" onClick={refresh}>
            Try again
          </button>
        </div>
      )}

      <section className="summary-grid" aria-label="Today summary">
        <article className="primary-stat">
          <p>Active time</p>
          <strong>{loading ? "—" : formatDuration(summary?.activeDurationMs ?? 0)}</strong>
          <span>
            <i className="live-indicator" />
            Recorded locally on this Mac
          </span>
        </article>

        <div className="secondary-stats">
          <article>
            <p>First activity</p>
            <strong>{formatClock(summary?.firstActivityAtMs ?? null)}</strong>
            <span>Started today</span>
          </article>
          <article>
            <p>Last activity</p>
            <strong>{formatClock(summary?.lastActivityAtMs ?? null)}</strong>
            <span>Latest checkpoint</span>
          </article>
          <article>
            <p>Idle</p>
            <strong>{formatDuration(summary?.idleDurationMs ?? 0)}</strong>
            <span>Away from computer</span>
          </article>
        </div>
      </section>

      {summary && (
        <ActivityDistribution
          timeline={timeline}
          startMs={summary.range.startMs}
          endMs={summary.range.endMs}
        />
      )}

      {!summary && !error && (
        <section className="activity-card skeleton-card" aria-label="Loading activity">
          <div className="skeleton title" />
          <div className="skeleton chart" />
        </section>
      )}

      <footer className="privacy-note">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M7 10V7a5 5 0 0 1 10 0v3M5 10h14v11H5z" />
        </svg>
        Activity stays private and is stored only on this Mac.
      </footer>
    </div>
  );
}
