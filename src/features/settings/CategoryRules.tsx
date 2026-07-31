import { FormEvent, useEffect, useState } from "react";
import {
  CategoryRule,
  CategoryRuleInput,
  CategoryRuleMatchField,
  createCategoryRule,
  deleteCategoryRule,
  errorMessage,
  getCategoryRules,
  reapplyCategoryRules,
  updateCategoryRule,
} from "../../lib/ipc";
import { notifyActivityDataChanged } from "../../lib/events";
import { useLocale } from "../../lib/i18n";
import "./CategoryRules.css";

interface CategoryRuleDraft {
  matchField: CategoryRuleMatchField;
  pattern: string;
  category: string;
  priority: string;
  enabled: boolean;
}

const EMPTY_DRAFT: CategoryRuleDraft = {
  matchField: "APPLICATION_NAME",
  pattern: "",
  category: "",
  priority: "100",
  enabled: true,
};

export function categoryRuleInputFromDraft(
  draft: CategoryRuleDraft,
): CategoryRuleInput | null {
  const pattern = draft.pattern.trim();
  const category = draft.category.trim();
  const priority = Number(draft.priority);
  if (
    pattern.length < 1
    || [...pattern].length > 120
    || category.length < 1
    || [...category].length > 40
    || !Number.isInteger(priority)
    || priority < 0
    || priority > 9999
  ) {
    return null;
  }
  return {
    matchField: draft.matchField,
    pattern,
    category,
    priority,
    enabled: draft.enabled,
  };
}

export function categoryRuleActionsLocked(
  saving: boolean,
  busyId: number | null,
  reapplying: boolean,
): boolean {
  return saving || busyId !== null || reapplying;
}

function draftFromRule(rule: CategoryRule): CategoryRuleDraft {
  return {
    matchField: rule.matchField,
    pattern: rule.pattern,
    category: rule.category,
    priority: String(rule.priority),
    enabled: rule.enabled,
  };
}

const MATCH_FIELD_LABELS: Record<CategoryRuleMatchField, string> = {
  APPLICATION_NAME: "Application name",
  BUNDLE_ID: "Bundle identifier",
  WINDOW_TITLE: "Window title",
};

function Toggle({ checked, disabled, label, onChange }: {
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

export function CategoryRules() {
  const { t } = useLocale();
  const [rules, setRules] = useState<CategoryRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [draft, setDraft] = useState<CategoryRuleDraft>(EMPTY_DRAFT);
  const [editorOpen, setEditorOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [reapplying, setReapplying] = useState(false);
  const actionsLocked = categoryRuleActionsLocked(saving, busyId, reapplying);

  async function loadRules() {
    setLoading(true);
    setLoadError(null);
    try {
      setRules(await getCategoryRules());
    } catch (error) {
      setLoadError(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadRules();
  }, []);

  function openCreateEditor() {
    setEditingId(null);
    setDraft(EMPTY_DRAFT);
    setNotice(null);
    setEditorOpen(true);
  }

  function openEditEditor(rule: CategoryRule) {
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
    const input = categoryRuleInputFromDraft(draft);
    if (!input) {
      setNotice("Enter a pattern, category, and priority from 0 to 9999.");
      return;
    }
    setSaving(true);
    setNotice(null);
    try {
      const saved = editingId === null
        ? await createCategoryRule(input)
        : await updateCategoryRule(editingId, input);
      setRules((current) => (editingId === null
        ? [...current, saved]
        : current.map((rule) => rule.id === editingId ? saved : rule))
        .sort((left, right) => left.priority - right.priority || left.id - right.id));
      setNotice(editingId === null ? "Classification rule added." : "Classification rule updated.");
      setEditorOpen(false);
      setEditingId(null);
      setDraft(EMPTY_DRAFT);
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setSaving(false);
    }
  }

  async function setRuleEnabled(rule: CategoryRule, enabled: boolean) {
    setBusyId(rule.id);
    setNotice(null);
    try {
      const updated = await updateCategoryRule(rule.id, { ...rule, enabled });
      setRules((current) => current.map((item) => item.id === rule.id ? updated : item));
      setNotice("Classification rule updated.");
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusyId(null);
    }
  }

  async function removeRule(rule: CategoryRule) {
    if (!window.confirm(`${t("Delete classification rule")} “${rule.pattern}”?`)) return;
    setBusyId(rule.id);
    setNotice(null);
    try {
      await deleteCategoryRule(rule.id);
      setRules((current) => current.filter((item) => item.id !== rule.id));
      if (editingId === rule.id) closeEditor();
      setNotice("Classification rule deleted.");
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusyId(null);
    }
  }

  async function reapply() {
    setReapplying(true);
    setNotice(null);
    try {
      const changed = await reapplyCategoryRules();
      setNotice(`${changed} sessions reclassified.`);
      notifyActivityDataChanged();
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setReapplying(false);
    }
  }

  return (
    <section className="settings-card category-rules-card">
      <div className="list-heading category-rules-heading">
        <div>
          <p className="section-kicker">{t("Automation")}</p>
          <h2>{t("Automatic classification")}</h2>
        </div>
        <button
          type="button"
          className="category-rule-add"
          disabled={loading || loadError !== null || actionsLocked}
          onClick={openCreateEditor}
        >
          {t("Add rule")}
        </button>
      </div>
      <p className="settings-note">
        {t("The first enabled rule by priority assigns a category to each activity session.")}
      </p>

      {loadError && !loading && (
        <div className="category-rule-load-error" role="alert">
          <div>
            <strong>{t("Unable to load classification rules.")}</strong>
            <small>{t(loadError)}</small>
          </div>
          <button type="button" onClick={() => void loadRules()}>{t("Retry")}</button>
        </div>
      )}

      {notice && (
        <p className="category-rule-notice" role="status">
          {t(notice)}
        </p>
      )}

      {editorOpen && (
        <form className="category-rule-editor" onSubmit={(event) => void saveRule(event)}>
          <div className="category-rule-editor-heading">
            <strong>{t(editingId === null ? "New classification rule" : "Edit classification rule")}</strong>
            <button type="button" disabled={saving} onClick={closeEditor}>{t("Close")}</button>
          </div>
          <div className="category-rule-form-grid">
            <label>
              <span>{t("Match field")}</span>
              <select
                value={draft.matchField}
                disabled={saving}
                onChange={(event) => setDraft({
                  ...draft,
                  matchField: event.currentTarget.value as CategoryRuleMatchField,
                })}
              >
                {(Object.keys(MATCH_FIELD_LABELS) as CategoryRuleMatchField[]).map((field) => (
                  <option value={field} key={field}>{t(MATCH_FIELD_LABELS[field])}</option>
                ))}
              </select>
            </label>
            <label>
              <span>{t("Contains")}</span>
              <input
                value={draft.pattern}
                maxLength={120}
                disabled={saving}
                placeholder={t("example.com, Visual Studio Code…")}
                onChange={(event) => setDraft({ ...draft, pattern: event.currentTarget.value })}
              />
            </label>
            <label>
              <span>{t("Category")}</span>
              <input
                value={draft.category}
                maxLength={40}
                disabled={saving}
                placeholder={t("Work, Communication, Learning…")}
                onChange={(event) => setDraft({ ...draft, category: event.currentTarget.value })}
              />
            </label>
            <label>
              <span>{t("Priority")}</span>
              <input
                type="number"
                min="0"
                max="9999"
                step="1"
                value={draft.priority}
                disabled={saving}
                onChange={(event) => setDraft({ ...draft, priority: event.currentTarget.value })}
              />
            </label>
          </div>
          <div className="category-rule-editor-footer">
            <label>
              <span>{t("Enable rule")}</span>
              <Toggle
                checked={draft.enabled}
                disabled={saving}
                label={t("Enable rule")}
                onChange={(enabled) => setDraft({ ...draft, enabled })}
              />
            </label>
            <button type="submit" disabled={saving || categoryRuleInputFromDraft(draft) === null}>
              {t(saving ? "Saving…" : editingId === null ? "Add rule" : "Save rule")}
            </button>
          </div>
        </form>
      )}

      {loading ? (
        <p className="category-rule-empty">{t("Loading classification rules…")}</p>
      ) : loadError ? null : rules.length === 0 ? (
        <div className="category-rule-empty">
          <strong>{t("No classification rules")}</strong>
          <span>{t("Add a rule to classify future activity by application, identifier, or window title.")}</span>
        </div>
      ) : (
        <div className="category-rules-list">
          {rules.map((rule) => (
            <article className={`category-rule${rule.enabled ? "" : " disabled"}`} key={rule.id}>
              <div className="category-rule-main">
                <div className="category-rule-title">
                  <span>{t(MATCH_FIELD_LABELS[rule.matchField])}</span>
                  <strong>{rule.pattern}</strong>
                  <i aria-hidden="true">→</i>
                  <b>{rule.category}</b>
                </div>
                <small>{t("Priority")} {rule.priority}</small>
              </div>
              <div className="category-rule-controls">
                <Toggle
                  checked={rule.enabled}
                  disabled={actionsLocked}
                  label={t("Enable rule")}
                  onChange={(enabled) => void setRuleEnabled(rule, enabled)}
                />
                <button disabled={actionsLocked} onClick={() => openEditEditor(rule)}>
                  {t("Edit")}
                </button>
                <button
                  className="delete"
                  disabled={actionsLocked}
                  onClick={() => void removeRule(rule)}
                >
                  {t("Delete")}
                </button>
              </div>
            </article>
          ))}
        </div>
      )}

      <div className="category-rule-history">
        <div>
          <strong>{t("Reclassify history")}</strong>
          <small>{t("Apply current rules to existing active sessions. Manual application categories remain unchanged.")}</small>
        </div>
        <button
          type="button"
          disabled={loading || loadError !== null || actionsLocked}
          onClick={() => void reapply()}
        >
          {t(reapplying ? "Reclassifying…" : "Apply to history")}
        </button>
      </div>
    </section>
  );
}
