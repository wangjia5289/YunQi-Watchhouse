import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  UsageLimitApplicationTarget,
  UsageLimitRule,
  UsageLimitRuleInput,
  UsageLimitScopeType,
  createUsageLimit,
  deleteUsageLimit,
  errorMessage,
  getUsageLimitTargets,
  getUsageLimits,
  updateUsageLimit,
} from "../../lib/ipc";
import { Locale, useLocale } from "../../lib/i18n";
import "./UsageLimits.css";

const DAY_MINUTES = 24 * 60;
const DEFAULT_LIMIT_MINUTES = 120;

interface UsageLimitDraft {
  scopeType: UsageLimitScopeType;
  applicationId: string;
  category: string;
  weekdayLimitMinutes: string;
  weekendLimitMinutes: string;
  notificationsEnabled: boolean;
  enabled: boolean;
}

const EMPTY_DRAFT: UsageLimitDraft = {
  scopeType: "APPLICATION",
  applicationId: "",
  category: "",
  weekdayLimitMinutes: String(DEFAULT_LIMIT_MINUTES),
  weekendLimitMinutes: String(DEFAULT_LIMIT_MINUTES),
  notificationsEnabled: true,
  enabled: true,
};

export function usageLimitInputFromDraft(
  draft: UsageLimitDraft,
): UsageLimitRuleInput | null {
  const weekdayLimitMinutes = Number(draft.weekdayLimitMinutes);
  const weekendLimitMinutes = Number(draft.weekendLimitMinutes);
  const validMinutes = [weekdayLimitMinutes, weekendLimitMinutes].every(
    (minutes) => Number.isInteger(minutes) && minutes >= 1 && minutes <= DAY_MINUTES,
  );
  if (!validMinutes) return null;

  const applicationId = Number(draft.applicationId);
  if (
    draft.scopeType === "APPLICATION"
    && (!Number.isInteger(applicationId) || applicationId <= 0)
  ) {
    return null;
  }
  const category = draft.category.trim();
  if (
    draft.scopeType === "CATEGORY"
    && (category.length < 1 || [...category].length > 40)
  ) {
    return null;
  }

  return {
    scopeType: draft.scopeType,
    applicationId: draft.scopeType === "APPLICATION" ? applicationId : null,
    category: draft.scopeType === "CATEGORY" ? category : null,
    weekdayLimitMinutes,
    weekendLimitMinutes,
    notificationsEnabled: draft.notificationsEnabled,
    enabled: draft.enabled,
  };
}

export function formatLimitMinutes(minutes: number, locale: Locale): string {
  const hours = Math.floor(minutes / 60);
  const remainder = minutes % 60;
  void locale;
  if (hours === 0) return `${remainder}min`;
  if (remainder === 0) return `${hours}h`;
  return `${hours}h ${remainder}min`;
}

function draftFromRule(rule: UsageLimitRule): UsageLimitDraft {
  return {
    scopeType: rule.scopeType,
    applicationId: rule.applicationId?.toString() ?? "",
    category: rule.category ?? "",
    weekdayLimitMinutes: String(rule.weekdayLimitMinutes),
    weekendLimitMinutes: String(rule.weekendLimitMinutes),
    notificationsEnabled: rule.notificationsEnabled,
    enabled: rule.enabled,
  };
}

function targetName(rule: UsageLimitRule): string {
  return rule.scopeType === "APPLICATION"
    ? rule.applicationName ?? ""
    : rule.category ?? "";
}

function Toggle({
  checked,
  disabled,
  label,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-label={label}
      aria-checked={checked}
      className={`settings-toggle${checked ? " active" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  );
}

export function UsageLimits() {
  const { locale, t } = useLocale();
  const [rules, setRules] = useState<UsageLimitRule[]>([]);
  const [applications, setApplications] = useState<UsageLimitApplicationTarget[]>([]);
  const [categories, setCategories] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [draft, setDraft] = useState<UsageLimitDraft>(EMPTY_DRAFT);
  const [editorOpen, setEditorOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [busyId, setBusyId] = useState<number | null>(null);

  async function load() {
    setLoading(true);
    setLoadError(null);
    try {
      const [nextRules, targets] = await Promise.all([
        getUsageLimits(),
        getUsageLimitTargets(),
      ]);
      setRules(nextRules);
      setApplications(targets.applications);
      setCategories(targets.categories);
    } catch (error) {
      setLoadError(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  const currentRule = editingId === null
    ? null
    : rules.find((rule) => rule.id === editingId) ?? null;

  const applicationOptions = useMemo(() => {
    const usedIds = new Set(
      rules
        .filter((rule) => rule.scopeType === "APPLICATION" && rule.id !== editingId)
        .map((rule) => rule.applicationId),
    );
    const options = applications.filter(
      (application) => !usedIds.has(application.applicationId),
    );
    if (
      currentRule?.scopeType === "APPLICATION"
      && currentRule.applicationId !== null
      && !options.some((application) => application.applicationId === currentRule.applicationId)
    ) {
      options.push({
        applicationId: currentRule.applicationId,
        applicationName: currentRule.applicationName ?? t("Unknown application"),
      });
    }
    return options.sort((left, right) =>
      left.applicationName.localeCompare(right.applicationName, locale));
  }, [applications, currentRule, editingId, locale, rules, t]);

  const categoryOptions = useMemo(() => {
    const usedCategories = new Set(
      rules
        .filter((rule) => rule.scopeType === "CATEGORY" && rule.id !== editingId)
        .map((rule) => rule.category?.toLocaleLowerCase()),
    );
    const values = new Set(categories);
    if (currentRule?.scopeType === "CATEGORY" && currentRule.category) {
      values.add(currentRule.category);
    }
    return [...values]
      .filter((category) => !usedCategories.has(category.toLocaleLowerCase()))
      .sort((left, right) => left.localeCompare(right, locale));
  }, [categories, currentRule, editingId, locale, rules]);

  function openCreateEditor() {
    const scopeType = applicationOptions.length > 0 ? "APPLICATION" : "CATEGORY";
    setEditingId(null);
    setDraft({
      ...EMPTY_DRAFT,
      scopeType,
      applicationId: applicationOptions[0]?.applicationId.toString() ?? "",
      category: categoryOptions[0] ?? "",
    });
    setNotice(null);
    setEditorOpen(true);
  }

  function openEditEditor(rule: UsageLimitRule) {
    setEditingId(rule.id);
    setDraft(draftFromRule(rule));
    setNotice(null);
    setEditorOpen(true);
  }

  function closeEditor() {
    if (saving) return;
    setEditorOpen(false);
    setEditingId(null);
    setDraft(EMPTY_DRAFT);
  }

  async function saveRule(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const input = usageLimitInputFromDraft(draft);
    if (!input) {
      setNotice("Choose a target and enter limits from 1 to 1440 minutes.");
      return;
    }
    setSaving(true);
    setNotice(null);
    try {
      const saved = editingId === null
        ? await createUsageLimit(input)
        : await updateUsageLimit(editingId, input);
      setRules((current) => editingId === null
        ? [...current, saved]
        : current.map((rule) => rule.id === editingId ? saved : rule));
      setNotice(editingId === null ? "Usage limit added." : "Usage limit updated.");
      setEditorOpen(false);
      setEditingId(null);
      setDraft(EMPTY_DRAFT);
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setSaving(false);
    }
  }

  async function patchRule(rule: UsageLimitRule, patch: Partial<UsageLimitRuleInput>) {
    setBusyId(rule.id);
    setNotice(null);
    try {
      const updated = await updateUsageLimit(rule.id, {
        scopeType: rule.scopeType,
        applicationId: rule.applicationId,
        category: rule.category,
        weekdayLimitMinutes: rule.weekdayLimitMinutes,
        weekendLimitMinutes: rule.weekendLimitMinutes,
        notificationsEnabled: rule.notificationsEnabled,
        enabled: rule.enabled,
        ...patch,
      });
      setRules((current) => current.map((item) => item.id === rule.id ? updated : item));
      setNotice("Usage limit updated.");
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusyId(null);
    }
  }

  async function removeRule(rule: UsageLimitRule) {
    const name = targetName(rule);
    if (!window.confirm(`${t("Delete the usage limit for")} “${name}”?`)) return;
    setBusyId(rule.id);
    setNotice(null);
    try {
      await deleteUsageLimit(rule.id);
      setRules((current) => current.filter((item) => item.id !== rule.id));
      if (editingId === rule.id) closeEditor();
      setNotice("Usage limit deleted.");
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusyId(null);
    }
  }

  const selectedOptions = draft.scopeType === "APPLICATION"
    ? applicationOptions.length
    : categoryOptions.length;
  const validDraft = usageLimitInputFromDraft(draft) !== null;

  return (
    <section className="settings-card usage-limits-card">
      <div className="list-heading usage-limits-heading">
        <div>
          <p className="section-kicker">{t("Wellbeing")}</p>
          <h2>{t("Application usage limits")}</h2>
        </div>
        <button
          type="button"
          className="usage-limit-add"
          disabled={
            loading
            || busyId !== null
            || (applicationOptions.length === 0 && categoryOptions.length === 0)
          }
          onClick={openCreateEditor}
        >
          {t("Add limit")}
        </button>
      </div>
      <p className="settings-note">
        {t("Set separate daily limits for weekdays and weekends. Alerts are sent at 80% and 100%.")}
      </p>

      {(notice || loadError) && (
        <div className={`usage-limit-notice${loadError ? " error" : ""}`}>
          <span>{t(notice ?? loadError ?? "")}</span>
          {loadError && (
            <button type="button" onClick={() => void load()}>
              {t("Retry")}
            </button>
          )}
        </div>
      )}

      {editorOpen && (
        <form className="usage-limit-editor" onSubmit={(event) => void saveRule(event)}>
          <div className="usage-limit-editor-heading">
            <strong>{t(editingId === null ? "New usage limit" : "Edit usage limit")}</strong>
            <button type="button" disabled={saving} onClick={closeEditor}>
              {t("Cancel")}
            </button>
          </div>

          <div className="usage-limit-scope" role="group" aria-label={t("Limit type")}>
            {(["APPLICATION", "CATEGORY"] as const).map((scopeType) => (
              <button
                type="button"
                className={draft.scopeType === scopeType ? "active" : ""}
                disabled={saving}
                key={scopeType}
                onClick={() => setDraft((current) => ({
                  ...current,
                  scopeType,
                  applicationId: scopeType === "APPLICATION"
                    ? applicationOptions[0]?.applicationId.toString() ?? ""
                    : "",
                  category: scopeType === "CATEGORY" ? categoryOptions[0] ?? "" : "",
                }))}
              >
                {t(scopeType === "APPLICATION" ? "Application" : "Category")}
              </button>
            ))}
          </div>

          <label className="usage-limit-field usage-limit-target">
            <span>{t(draft.scopeType === "APPLICATION" ? "Application" : "Category")}</span>
            <select
              value={draft.scopeType === "APPLICATION" ? draft.applicationId : draft.category}
              disabled={saving || selectedOptions === 0}
              onChange={(event) => setDraft((current) => draft.scopeType === "APPLICATION"
                ? { ...current, applicationId: event.currentTarget.value }
                : { ...current, category: event.currentTarget.value })}
            >
              {selectedOptions === 0 && <option value="">{t("No available targets")}</option>}
              {draft.scopeType === "APPLICATION"
                ? applicationOptions.map((application) => (
                  <option value={application.applicationId} key={application.applicationId}>
                    {application.applicationName}
                  </option>
                ))
                : categoryOptions.map((category) => (
                  <option value={category} key={category}>{category}</option>
                ))}
            </select>
          </label>

          <div className="usage-limit-duration-grid">
            <label className="usage-limit-field">
              <span>{t("Weekday daily limit")}</span>
              <span className="usage-limit-number">
                <input
                  type="number"
                  min={1}
                  max={DAY_MINUTES}
                  step={1}
                  value={draft.weekdayLimitMinutes}
                  disabled={saving}
                  onChange={(event) => setDraft((current) => ({
                    ...current,
                    weekdayLimitMinutes: event.currentTarget.value,
                  }))}
                />
                <small>{t("minutes")}</small>
              </span>
            </label>
            <label className="usage-limit-field">
              <span>{t("Weekend daily limit")}</span>
              <span className="usage-limit-number">
                <input
                  type="number"
                  min={1}
                  max={DAY_MINUTES}
                  step={1}
                  value={draft.weekendLimitMinutes}
                  disabled={saving}
                  onChange={(event) => setDraft((current) => ({
                    ...current,
                    weekendLimitMinutes: event.currentTarget.value,
                  }))}
                />
                <small>{t("minutes")}</small>
              </span>
            </label>
          </div>

          <div className="usage-limit-options">
            <div>
              <span>{t("Enable limit")}</span>
              <Toggle
                checked={draft.enabled}
                disabled={saving}
                label={t("Enable limit")}
                onChange={(enabled) => setDraft((current) => ({ ...current, enabled }))}
              />
            </div>
            <div>
              <span>{t("Limit notifications")}</span>
              <Toggle
                checked={draft.notificationsEnabled}
                disabled={saving}
                label={t("Limit notifications")}
                onChange={(notificationsEnabled) => setDraft((current) => ({
                  ...current,
                  notificationsEnabled,
                }))}
              />
            </div>
          </div>

          <div className="usage-limit-form-actions">
            <button type="submit" disabled={saving || !validDraft}>
              {t(saving ? "Saving…" : editingId === null ? "Add limit" : "Save changes")}
            </button>
          </div>
        </form>
      )}

      {loading ? (
        <p className="usage-limit-empty">{t("Loading usage limits…")}</p>
      ) : rules.length === 0 ? (
        <div className="usage-limit-empty">
          <strong>{t("No usage limits")}</strong>
          <span>{t("Add a limit to receive a reminder before an application or category reaches its daily allowance.")}</span>
        </div>
      ) : (
        <div className="usage-limit-rules-list">
          {rules.map((rule) => {
            const name = targetName(rule);
            const isBusy = busyId !== null;
            return (
              <article className={`usage-limit-rule${rule.enabled ? "" : " disabled"}`} key={rule.id}>
                <div className="usage-limit-rule-main">
                  <div className="usage-limit-rule-title">
                    <span>{t(rule.scopeType === "APPLICATION" ? "Application" : "Category")}</span>
                    <strong>{name}</strong>
                  </div>
                  <div className="usage-limit-rule-schedule">
                    <span>
                      {t("Weekdays")}
                      <strong>{formatLimitMinutes(rule.weekdayLimitMinutes, locale)}</strong>
                    </span>
                    <span>
                      {t("Weekends")}
                      <strong>{formatLimitMinutes(rule.weekendLimitMinutes, locale)}</strong>
                    </span>
                  </div>
                </div>

                <div className="usage-limit-rule-controls">
                  <label>
                    <span>{t("Enabled")}</span>
                    <Toggle
                      checked={rule.enabled}
                      disabled={isBusy}
                      label={`${t("Enable limit")}: ${name}`}
                      onChange={(enabled) => void patchRule(rule, { enabled })}
                    />
                  </label>
                  <label>
                    <span>{t("Alerts")}</span>
                    <Toggle
                      checked={rule.notificationsEnabled}
                      disabled={isBusy}
                      label={`${t("Limit notifications")}: ${name}`}
                      onChange={(notificationsEnabled) =>
                        void patchRule(rule, { notificationsEnabled })}
                    />
                  </label>
                  <div className="usage-limit-rule-actions">
                    <button type="button" disabled={isBusy} onClick={() => openEditEditor(rule)}>
                      {t("Edit")}
                    </button>
                    <button
                      type="button"
                      className="delete"
                      disabled={isBusy}
                      onClick={() => void removeRule(rule)}
                    >
                      {t("Delete")}
                    </button>
                  </div>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}
