import { useEffect, useMemo, useState } from "react";
import { formatDuration } from "../../lib/format";
import {
  FocusPlanHistorySummary,
  ProductivityReport,
  WeeklyReportArchive,
  archiveWeeklyReport,
  deleteWeeklyReportArchive,
  errorMessage,
  getWeeklyReportArchives,
  sendWeeklyReportNotification,
} from "../../lib/ipc";
import { useLocale } from "../../lib/i18n";
import { buildWeeklyArchiveInput, weeklyArchiveIsComplete } from "./weeklyArchiveModel";
import "./WeeklyArchivePanel.css";

export function WeeklyArchivePanel({
  report,
  focusHistory,
  previousWeekActiveDurationMs,
}: {
  report: ProductivityReport;
  focusHistory: FocusPlanHistorySummary | null;
  previousWeekActiveDurationMs: number;
}) {
  const { locale, t } = useLocale();
  const dateLocale = locale === "zh-CN" ? "zh-CN" : "en";
  const [archives, setArchives] = useState<WeeklyReportArchive[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const input = useMemo(
    () => buildWeeklyArchiveInput(report, focusHistory, previousWeekActiveDurationMs),
    [focusHistory, previousWeekActiveDurationMs, report],
  );

  async function loadArchives() {
    setArchives(await getWeeklyReportArchives(12));
  }

  useEffect(() => {
    void loadArchives().catch((reason) => setMessage(errorMessage(reason)));
  }, []);

  async function saveArchive() {
    setBusy("save");
    setMessage(null);
    try {
      await archiveWeeklyReport(input);
      await loadArchives();
      setMessage(t("Weekly report archived locally."));
    } catch (reason) {
      setMessage(t(errorMessage(reason)));
    } finally {
      setBusy(null);
    }
  }

  async function notify(weekStartDate: string) {
    setBusy(`notify-${weekStartDate}`);
    setMessage(null);
    try {
      await sendWeeklyReportNotification(weekStartDate);
      await loadArchives();
      setMessage(t("Weekly report notification sent."));
    } catch (reason) {
      setMessage(t(errorMessage(reason)));
    } finally {
      setBusy(null);
    }
  }

  async function remove(weekStartDate: string) {
    if (!window.confirm(t("Delete this archived weekly report?"))) return;
    setBusy(`delete-${weekStartDate}`);
    setMessage(null);
    try {
      await deleteWeeklyReportArchive(weekStartDate);
      await loadArchives();
      setMessage(t("Archived weekly report deleted."));
    } catch (reason) {
      setMessage(t(errorMessage(reason)));
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="report-section weekly-archive" aria-labelledby="weekly-archive-title">
      <div className="section-heading">
        <div>
          <p className="section-kicker">{t("History")}</p>
          <h2 id="weekly-archive-title">{t("Weekly report archive")}</h2>
        </div>
        <button type="button" disabled={busy !== null} onClick={() => void saveArchive()}>
          {t(busy === "save" ? "Archiving…" : "Save current snapshot")}
        </button>
      </div>
      <p className="weekly-archive-note">
        {t("Automatic weekly archives finalize current-week snapshots after the week ends.")}
      </p>
      {message && <p className="weekly-archive-message" role="status">{message}</p>}
      {archives.length ? (
        <div className="weekly-archive-list">
          {archives.map((archive) => (
            <article key={archive.weekStartDate}>
              <div>
                <strong>{new Date(`${archive.weekStartDate}T12:00:00`).toLocaleDateString(dateLocale, {
                  month: "short",
                  day: "numeric",
                })} - {new Date(`${archive.weekEndDate}T12:00:00`).toLocaleDateString(dateLocale, {
                  month: "short",
                  day: "numeric",
                })}</strong>
                <small>
                  {formatDuration(archive.activeDurationMs, locale)} {t("active")}
                  {!weeklyArchiveIsComplete(archive) && ` · ${t("In progress")}`}
                </small>
              </div>
              <span>{archive.leadingCategory ?? t("No category data")}</span>
              <span>{archive.focusCompletionRate === null ? "-" : `${archive.focusCompletionRate}%`} {t("focus completion")}</span>
              <div className="weekly-archive-actions">
                <button
                  type="button"
                  disabled={busy !== null}
                  onClick={() => void notify(archive.weekStartDate)}
                >{t(archive.notifiedAtMs ? "Notify again" : "Notify")}</button>
                <button
                  type="button"
                  className="danger"
                  disabled={busy !== null}
                  aria-label={t("Delete archived weekly report")}
                  onClick={() => void remove(archive.weekStartDate)}
                >{t("Delete")}</button>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <p className="report-empty">{t("No weekly reports have been archived yet.")}</p>
      )}
    </section>
  );
}
