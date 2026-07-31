import { useCallback, useEffect, useState } from "react";
import {
  UsageLimitDailyException,
  UsageLimitProgress,
  UsageLimitReminderHistoryEntry,
  addTemporaryUsageLimitMinutes,
  clearTemporaryUsageLimitMinutes,
  errorMessage,
  getTodayUsageLimitProgress,
  getUsageLimitReminderHistory,
  silenceUsageLimitNotificationsForToday,
  snoozeUsageLimitNotifications,
} from "../../lib/ipc";
import { notifyActivityDataChanged } from "../../lib/events";
import { dateFromLocalIso } from "../../lib/format";
import { Locale, useLocale } from "../../lib/i18n";
import { formatLimitMinutes } from "./UsageLimits";
import "./UsageLimitReminderCenter.css";

const HISTORY_DAY_OPTIONS = [7, 30] as const;
const TEMPORARY_MINUTE_OPTIONS = [15, 30, 60] as const;

type ReminderAction = "snooze" | "silence" | "add" | "clear";

function targetName(
  item: Pick<UsageLimitProgress, "scopeType" | "applicationName" | "category">
    | UsageLimitReminderHistoryEntry,
): string {
  if ("targetName" in item && item.targetName) return item.targetName;
  return item.scopeType === "APPLICATION"
    ? item.applicationName ?? ""
    : item.category ?? "";
}

export function formatReminderLocalDate(value: string, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium" })
    .format(dateFromLocalIso(value));
}

export function formatReminderDeliveryTime(timestamp: number, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" })
    .format(timestamp);
}

export function hasActiveUsageLimitSnooze(
  snoozedUntilMs: number | null,
  nowMs = Date.now(),
): snoozedUntilMs is number {
  return snoozedUntilMs !== null && snoozedUntilMs > nowMs;
}

export function applyUsageLimitDailyException(
  item: UsageLimitProgress,
  exception: UsageLimitDailyException,
): UsageLimitProgress {
  if (item.id !== exception.ruleId) return item;
  return {
    ...item,
    limitMinutes: item.baseLimitMinutes + exception.temporaryAddedMinutes,
    temporaryAddedMinutes: exception.temporaryAddedMinutes,
    notificationsSnoozedUntilMs: exception.notificationsSnoozedUntilMs,
    notificationsSilenced: exception.notificationsSilenced,
  };
}

export function UsageLimitReminderCenter() {
  const { locale, t } = useLocale();
  const [historyDays, setHistoryDays] = useState<(typeof HISTORY_DAY_OPTIONS)[number]>(7);
  const [history, setHistory] = useState<UsageLimitReminderHistoryEntry[]>([]);
  const [todayProgress, setTodayProgress] = useState<UsageLimitProgress[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [temporaryMinutes, setTemporaryMinutes] = useState<Record<number, number>>({});

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const [nextHistory, nextProgress] = await Promise.all([
        getUsageLimitReminderHistory(historyDays),
        getTodayUsageLimitProgress(),
      ]);
      setHistory(nextHistory);
      setTodayProgress(nextProgress);
    } catch (error) {
      setLoadError(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }, [historyDays]);

  useEffect(() => {
    void load();
  }, [load]);

  async function applyException(
    ruleId: number,
    action: ReminderAction,
    execute: () => Promise<UsageLimitDailyException>,
    successMessage: string,
  ) {
    const key = `${ruleId}:${action}`;
    setBusyAction(key);
    setNotice(null);
    try {
      const exception = await execute();
      setTodayProgress((current) => current.map((item) => applyUsageLimitDailyException(item, exception)));
      notifyActivityDataChanged();
      setNotice(successMessage);
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }

  function isBusy(ruleId: number, action: ReminderAction): boolean {
    return busyAction === `${ruleId}:${action}`;
  }

  return (
    <section className="settings-card usage-limit-reminders-card">
      <div className="list-heading usage-limit-reminders-heading">
        <div>
          <p className="section-kicker">{t("Wellbeing")}</p>
          <h2>{t("Usage limit reminders")}</h2>
        </div>
        <label className="usage-limit-history-range">
          <span>{t("History")}</span>
          <select
            aria-label={t("Reminder history range")}
            value={historyDays}
            disabled={loading || busyAction !== null}
            onChange={(event) => setHistoryDays(Number(event.currentTarget.value) as 7 | 30)}
          >
            {HISTORY_DAY_OPTIONS.map((days) => (
              <option value={days} key={days}>{t(days === 7 ? "Last 7 days" : "Last 30 days")}</option>
            ))}
          </select>
        </label>
      </div>
      <p className="settings-note">
        {t("Manage today’s reminder exceptions without changing your regular usage limits.")}
      </p>

      {(notice || loadError) && (
        <div className={`usage-limit-reminders-notice${loadError ? " error" : ""}`} role="status">
          <span>{t(notice ?? loadError ?? "")}</span>
          {loadError && (
            <button type="button" onClick={() => void load()}>{t("Retry")}</button>
          )}
        </div>
      )}

      <div className="usage-limit-reminders-section">
        <div className="usage-limit-reminders-section-heading">
          <h3>{t("Today’s rules")}</h3>
          <span>{t("Temporary changes reset tomorrow.")}</span>
        </div>
        {loading ? (
          <p className="usage-limit-reminders-empty">{t("Loading reminder controls…")}</p>
        ) : todayProgress.length === 0 ? (
          <div className="usage-limit-reminders-empty">
            <strong>{t("No usage limits for today")}</strong>
            <span>{t("Add a usage limit to manage its reminders here.")}</span>
          </div>
        ) : (
          <div className="usage-limit-reminders-list">
            {todayProgress.map((rule) => {
              const name = targetName(rule);
              const selectedMinutes = temporaryMinutes[rule.id] ?? 30;
              const disabled = busyAction !== null || !rule.enabled;
              const notificationsDisabled = disabled || !rule.notificationsEnabled;
              return (
                <article className={`usage-limit-reminder-rule${rule.enabled ? "" : " disabled"}`} key={rule.id}>
                  <div className="usage-limit-reminder-rule-summary">
                    <div className="usage-limit-reminder-rule-title">
                      <span>{t(rule.scopeType === "APPLICATION" ? "Application" : "Category")}</span>
                      <strong title={name}>{name}</strong>
                    </div>
                    <p>
                      {t("Today’s limit")}: {formatLimitMinutes(rule.limitMinutes, locale)}
                      {rule.temporaryAddedMinutes > 0 && (
                        <span className="usage-limit-reminder-extra">
                          {" · "}{t("Includes")}{" "}{formatLimitMinutes(rule.temporaryAddedMinutes, locale)} {t("extra today")}
                        </span>
                      )}
                    </p>
                    {!rule.enabled && <small>{t("This rule is disabled. Enable it above to manage today’s exceptions.")}</small>}
                    {rule.notificationsSilenced && <small>{t("Reminders are muted for today.")}</small>}
                    {!rule.notificationsSilenced && hasActiveUsageLimitSnooze(
                      rule.notificationsSnoozedUntilMs,
                    ) && (
                      <small>{t("Reminders delayed until")} {formatReminderDeliveryTime(rule.notificationsSnoozedUntilMs, locale)}</small>
                    )}
                  </div>
                  <div className="usage-limit-reminder-rule-actions">
                    <div className="usage-limit-reminder-notification-actions">
                      <button
                        type="button"
                        disabled={notificationsDisabled}
                        onClick={() => void applyException(
                          rule.id,
                          "snooze",
                          () => snoozeUsageLimitNotifications(rule.id, 30),
                          "Reminders delayed for 30 minutes.",
                        )}
                      >
                        {t(isBusy(rule.id, "snooze") ? "Delaying…" : "Delay 30 minutes")}
                      </button>
                      <button
                        type="button"
                        disabled={notificationsDisabled || rule.notificationsSilenced}
                        onClick={() => void applyException(
                          rule.id,
                          "silence",
                          () => silenceUsageLimitNotificationsForToday(rule.id),
                          "Reminders muted for today.",
                        )}
                      >
                        {t(isBusy(rule.id, "silence") ? "Muting…" : "Mute for today")}
                      </button>
                    </div>
                    <div className="usage-limit-reminder-allowance-actions">
                      <label>
                        <span>{t("Extra time")}</span>
                        <select
                          aria-label={`${t("Extra time")}: ${name}`}
                          value={selectedMinutes}
                          disabled={disabled}
                          onChange={(event) => setTemporaryMinutes((current) => ({
                            ...current,
                            [rule.id]: Number(event.currentTarget.value),
                          }))}
                        >
                          {TEMPORARY_MINUTE_OPTIONS.map((minutes) => (
                            <option value={minutes} key={minutes}>{formatLimitMinutes(minutes, locale)}</option>
                          ))}
                        </select>
                      </label>
                      <button
                        type="button"
                        disabled={disabled}
                        onClick={() => void applyException(
                          rule.id,
                          "add",
                          () => addTemporaryUsageLimitMinutes(rule.id, selectedMinutes),
                          "Temporary time added for today.",
                        )}
                      >
                        {t(isBusy(rule.id, "add") ? "Adding…" : "Add time")}
                      </button>
                      <button
                        type="button"
                        className="clear"
                        disabled={disabled || rule.temporaryAddedMinutes === 0}
                        onClick={() => void applyException(
                          rule.id,
                          "clear",
                          () => clearTemporaryUsageLimitMinutes(rule.id),
                          "Temporary time cleared.",
                        )}
                      >
                        {t(isBusy(rule.id, "clear") ? "Clearing…" : "Clear extra time")}
                      </button>
                    </div>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </div>

      <div className="usage-limit-reminders-section usage-limit-reminder-history">
        <div className="usage-limit-reminders-section-heading">
          <h3>{t("Recent reminder history")}</h3>
          <span>{t("Only delivered reminders are shown.")}</span>
        </div>
        {loading ? (
          <p className="usage-limit-reminders-empty">{t("Loading reminder history…")}</p>
        ) : history.length === 0 ? (
          <div className="usage-limit-reminders-empty">
            <strong>{t("No reminder history")}</strong>
            <span>{t("Delivered 80% and 100% usage limit reminders will appear here.")}</span>
          </div>
        ) : (
          <div className="usage-limit-reminder-history-list">
            {history.map((entry) => (
              <article className="usage-limit-reminder-history-entry" key={`${entry.ruleId}-${entry.localDate}-${entry.threshold}`}>
                <div>
                  <div className="usage-limit-reminder-rule-title">
                    <span>{t(entry.scopeType === "APPLICATION" ? "Application" : "Category")}</span>
                    <strong title={targetName(entry)}>{targetName(entry)}</strong>
                  </div>
                  <p>{formatReminderLocalDate(entry.localDate, locale)}</p>
                </div>
                <div className="usage-limit-reminder-history-meta">
                  <strong>{entry.threshold}%</strong>
                  <time dateTime={new Date(entry.deliveredAtMs).toISOString()}>
                    {formatReminderDeliveryTime(entry.deliveredAtMs, locale)}
                  </time>
                </div>
              </article>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}
