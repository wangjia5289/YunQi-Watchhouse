import { DragEvent, FormEvent, useEffect, useState } from "react";
import {
  CategoryRule,
  CategoryRuleInput,
  CategoryRuleMatchField,
  CategoryRulePreview,
  CategoryRulePreviewSample,
  CategoryRulesReapplyPreview,
  CategoryRulesReapplyUndoStatus,
  createCategoryRule,
  deleteCategoryRule,
  errorMessage,
  getCategoryRules,
  getCategoryRulesReapplyUndoStatus,
  previewCategoryRule,
  previewCategoryRulesReapply,
  reapplyCategoryRules,
  reorderCategoryRules,
  undoCategoryRulesReapply,
  updateCategoryRule,
} from "../../lib/ipc";
import { notifyActivityDataChanged } from "../../lib/events";
import { useLocale } from "../../lib/i18n";
import { categoryRulePreviewState } from "./categoryRulePreviewModel";
import { moveCategoryRuleId, offsetCategoryRuleId } from "./categoryRuleOrderModel";
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

function previewSampleValue(
  sample: CategoryRulePreviewSample,
  matchField: CategoryRuleMatchField,
): string {
  if (matchField === "BUNDLE_ID") return sample.bundleId ?? "";
  if (matchField === "WINDOW_TITLE") return sample.windowTitle ?? "";
  return sample.bundleId ?? sample.windowTitle ?? "";
}

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
  const [reapplyPreview, setReapplyPreview] = useState<CategoryRulesReapplyPreview | null>(null);
  const [reapplyPreviewing, setReapplyPreviewing] = useState(false);
  const [reapplyUndo, setReapplyUndo] = useState<CategoryRulesReapplyUndoStatus | null>(null);
  const [undoingReapply, setUndoingReapply] = useState(false);
  const [draggedRuleId, setDraggedRuleId] = useState<number | null>(null);
  const [dragOverRuleId, setDragOverRuleId] = useState<number | null>(null);
  const [preview, setPreview] = useState<CategoryRulePreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const actionsLocked = categoryRuleActionsLocked(saving, busyId, reapplying)
    || reapplyPreviewing
    || undoingReapply;
  const orderingLocked = actionsLocked || editorOpen;

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
    void getCategoryRulesReapplyUndoStatus()
      .then(setReapplyUndo)
      .catch(() => setReapplyUndo(null));
  }, []);

  useEffect(() => {
    if (!reapplyUndo) return undefined;
    const remainingMs = reapplyUndo.expiresAtMs - Date.now();
    if (remainingMs <= 0) {
      setReapplyUndo(null);
      return undefined;
    }
    const timer = window.setTimeout(() => setReapplyUndo(null), remainingMs);
    return () => window.clearTimeout(timer);
  }, [reapplyUndo]);

  useEffect(() => {
    const input = categoryRuleInputFromDraft(draft);
    if (!editorOpen || !input) {
      setPreview(null);
      setPreviewLoading(false);
      setPreviewError(null);
      return undefined;
    }

    let cancelled = false;
    setPreview(null);
    setPreviewLoading(true);
    setPreviewError(null);
    const timer = window.setTimeout(() => {
      void previewCategoryRule(input, editingId)
        .then((result) => {
          if (!cancelled) setPreview(result);
        })
        .catch((error) => {
          if (!cancelled) setPreviewError(errorMessage(error));
        })
        .finally(() => {
          if (!cancelled) setPreviewLoading(false);
        });
    }, 350);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [draft, editingId, editorOpen]);

  function openCreateEditor() {
    setEditingId(null);
    setDraft(EMPTY_DRAFT);
    setNotice(null);
    setEditorOpen(true);
  }

  function invalidateReapplyPreview() {
    setReapplyPreview(null);
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
      invalidateReapplyPreview();
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
      invalidateReapplyPreview();
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
      invalidateReapplyPreview();
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusyId(null);
    }
  }

  async function applyRuleOrder(ruleIds: number[], movingRuleId: number) {
    if (ruleIds.every((ruleId, index) => ruleId === rules[index]?.id)) return;
    setBusyId(movingRuleId);
    setNotice(null);
    try {
      setRules(await reorderCategoryRules(ruleIds));
      invalidateReapplyPreview();
      setNotice("Classification rule order updated.");
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setBusyId(null);
      setDraggedRuleId(null);
      setDragOverRuleId(null);
    }
  }

  function moveRule(ruleId: number, offset: -1 | 1) {
    const nextIds = offsetCategoryRuleId(rules.map((rule) => rule.id), ruleId, offset);
    void applyRuleOrder(nextIds, ruleId);
  }

  function startRuleDrag(event: DragEvent<HTMLButtonElement>, ruleId: number) {
    if (orderingLocked) {
      event.preventDefault();
      return;
    }
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", String(ruleId));
    setDraggedRuleId(ruleId);
  }

  function dropRule(event: DragEvent<HTMLElement>, targetRuleId: number) {
    event.preventDefault();
    const sourceRuleId = draggedRuleId ?? Number(event.dataTransfer.getData("text/plain"));
    if (!Number.isInteger(sourceRuleId)) return;
    const nextIds = moveCategoryRuleId(
      rules.map((rule) => rule.id),
      sourceRuleId,
      targetRuleId,
    );
    void applyRuleOrder(nextIds, sourceRuleId);
  }

  async function previewReapply() {
    setReapplyPreviewing(true);
    setNotice(null);
    try {
      setReapplyPreview(await previewCategoryRulesReapply());
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setReapplyPreviewing(false);
    }
  }

  async function reapply() {
    setReapplying(true);
    setNotice(null);
    try {
      const result = await reapplyCategoryRules();
      setNotice(`${result.affectedCount} sessions reclassified.`);
      setReapplyPreview(null);
      setReapplyUndo(
        result.undoToken !== null
        && result.undoCreatedAtMs !== null
        && result.undoExpiresAtMs !== null
          ? {
            token: result.undoToken,
            createdAtMs: result.undoCreatedAtMs,
            expiresAtMs: result.undoExpiresAtMs,
            affectedCount: result.affectedCount,
          }
          : null,
      );
      notifyActivityDataChanged();
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setReapplying(false);
    }
  }

  async function undoReapply() {
    if (!reapplyUndo) return;
    setUndoingReapply(true);
    setNotice(null);
    try {
      const restored = await undoCategoryRulesReapply(reapplyUndo.token);
      setReapplyUndo(null);
      setReapplyPreview(null);
      setNotice(`${restored} reclassified sessions restored.`);
      notifyActivityDataChanged();
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setUndoingReapply(false);
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
      <p className="settings-note category-rule-order-note">
        {t("Drag rules or use the arrow buttons to change which rule runs first.")}
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
          <div className="category-rule-preview" aria-live="polite" aria-busy={previewLoading}>
            <div className="category-rule-preview-heading">
              <strong>{t("Match preview")}</strong>
              {previewLoading && <small>{t("Checking matches…")}</small>}
            </div>
            {previewError ? (
              <p className="category-rule-preview-error">{t(previewError)}</p>
            ) : preview ? (
              <>
                <div className="category-rule-preview-metrics">
                  <span>
                    <strong>{preview.matchedSessionCount}</strong>
                    <small>{t("Matching sessions")}</small>
                  </span>
                  <span>
                    <strong>{preview.matchedApplicationCount}</strong>
                    <small>{t("Applications")}</small>
                  </span>
                  <span>
                    <strong>{preview.effectiveSessionCount}</strong>
                    <small>{t("Will apply")}</small>
                  </span>
                </div>
                <p className={`category-rule-preview-status ${categoryRulePreviewState(preview, draft.enabled).toLowerCase()}`}>
                  {t({
                    NO_MATCHES: "No recorded sessions match this rule yet.",
                    DISABLED: "This rule is disabled; matches will not be classified.",
                    FULLY_SHADOWED: "Every match is already handled by an earlier rule.",
                    PARTIALLY_SHADOWED: "Some matches are already handled by earlier rules.",
                    WILL_APPLY: "This rule can classify every matching session.",
                  }[categoryRulePreviewState(preview, draft.enabled)])}
                </p>
                {preview.conflicts.length > 0 && (
                  <div className="category-rule-conflicts">
                    <strong>{t("Earlier matching rules")}</strong>
                    {preview.conflicts.map((conflict) => (
                      <div key={conflict.ruleId}>
                        <span>{t("Priority")} {conflict.priority}</span>
                        <b>{conflict.pattern}</b>
                        <i aria-hidden="true">→</i>
                        <em>{conflict.category}</em>
                        <small>{conflict.sessionCount} {t("sessions shadowed")}</small>
                      </div>
                    ))}
                  </div>
                )}
                {preview.samples.length > 0 && (
                  <div className="category-rule-preview-samples">
                    <strong>{t("Recent matches")}</strong>
                    {preview.samples.map((sample, index) => (
                      <div key={`${sample.applicationName}-${index}`}>
                        <span>
                          <b>{sample.applicationName}</b>
                          {previewSampleValue(sample, draft.matchField) && (
                            <small title={previewSampleValue(sample, draft.matchField)}>
                              {previewSampleValue(sample, draft.matchField)}
                            </small>
                          )}
                        </span>
                        <i className={draft.enabled ? (sample.wouldApply ? "applies" : "shadowed") : "disabled"}>
                          {t(!draft.enabled
                            ? "Rule disabled"
                            : sample.wouldApply
                              ? "Will apply"
                              : "Handled earlier")}
                        </i>
                      </div>
                    ))}
                  </div>
                )}
              </>
            ) : !previewLoading ? (
              <p>{t("Complete the rule to preview its matches.")}</p>
            ) : null}
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
          {rules.map((rule, index) => (
            <article
              className={`category-rule${rule.enabled ? "" : " disabled"}${dragOverRuleId === rule.id ? " drag-over" : ""}`}
              key={rule.id}
              onDragOver={(event) => {
                if (orderingLocked || draggedRuleId === rule.id) return;
                event.preventDefault();
                event.dataTransfer.dropEffect = "move";
                setDragOverRuleId(rule.id);
              }}
              onDragLeave={() => setDragOverRuleId((current) => current === rule.id ? null : current)}
              onDrop={(event) => dropRule(event, rule.id)}
            >
              <button
                type="button"
                className="category-rule-drag-handle"
                draggable={!orderingLocked}
                disabled={orderingLocked}
                aria-label={t("Drag to reorder rule")}
                title={t("Drag to reorder rule")}
                onDragStart={(event) => startRuleDrag(event, rule.id)}
                onDragEnd={() => {
                  setDraggedRuleId(null);
                  setDragOverRuleId(null);
                }}
              >
                <span aria-hidden="true">⋮⋮</span>
              </button>
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
                <button
                  type="button"
                  className="category-rule-order-button"
                  disabled={orderingLocked || index === 0}
                  aria-label={t("Move rule up")}
                  title={t("Move rule up")}
                  onClick={() => moveRule(rule.id, -1)}
                >
                  <span aria-hidden="true">↑</span>
                </button>
                <button
                  type="button"
                  className="category-rule-order-button"
                  disabled={orderingLocked || index === rules.length - 1}
                  aria-label={t("Move rule down")}
                  title={t("Move rule down")}
                  onClick={() => moveRule(rule.id, 1)}
                >
                  <span aria-hidden="true">↓</span>
                </button>
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
        <div className="category-rule-history-actions">
          <button
            type="button"
            disabled={loading || loadError !== null || actionsLocked}
            onClick={() => void previewReapply()}
          >
            {t(reapplyPreviewing ? "Previewing…" : "Preview changes")}
          </button>
          <button
            type="button"
            className="primary"
            disabled={
              loading
              || loadError !== null
              || actionsLocked
              || reapplyPreview === null
              || reapplyPreview.affectedSessionCount === 0
            }
            onClick={() => void reapply()}
          >
            {t(reapplying ? "Reclassifying…" : "Apply to history")}
          </button>
        </div>
      </div>

      {reapplyPreview && (
        <div className="category-rule-reapply-preview" aria-live="polite">
          <div className="category-rule-preview-heading">
            <strong>{t("Reclassification impact")}</strong>
            <small>{reapplyPreview.scannedSessionCount} {t("active sessions scanned")}</small>
          </div>
          <div className="category-rule-preview-metrics">
            <span>
              <strong>{reapplyPreview.affectedSessionCount}</strong>
              <small>{t("Sessions updated")}</small>
            </span>
            <span>
              <strong>{reapplyPreview.categoryChangeCount}</strong>
              <small>{t("Visible category changes")}</small>
            </span>
            <span>
              <strong>{reapplyPreview.assignedSessionCount}</strong>
              <small>{t("Assigned by rules")}</small>
            </span>
            <span>
              <strong>{reapplyPreview.clearedSessionCount}</strong>
              <small>{t("Reset to app default")}</small>
            </span>
          </div>
          {reapplyPreview.affectedSessionCount === 0 ? (
            <p>{t("History already matches the current rules.")}</p>
          ) : reapplyPreview.samples.length > 0 ? (
            <div className="category-rule-reapply-samples">
              <strong>{t("Recent changes")}</strong>
              {reapplyPreview.samples.map((sample, index) => (
                <div key={`${sample.applicationName}-${index}`}>
                  <span className="category-rule-reapply-app">
                    <b>{sample.applicationName}</b>
                    {sample.windowTitle && <small title={sample.windowTitle}>{sample.windowTitle}</small>}
                  </span>
                  <span className="category-rule-reapply-change">
                    <span>
                      <small>{t(sample.previousIsOverride ? "Rule" : "App default")}</small>
                      <b>{t(sample.previousCategory)}</b>
                    </span>
                    <i aria-hidden="true">→</i>
                    <span>
                      <small>{t(sample.nextIsOverride ? "Rule" : "App default")}</small>
                      <b>{t(sample.nextCategory)}</b>
                    </span>
                  </span>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      )}

      {reapplyUndo && reapplyUndo.expiresAtMs > Date.now() && (
        <div className="category-rule-undo" role="status">
          <div>
            <strong>{t("Reclassification can be undone")}</strong>
            <small>{reapplyUndo.affectedCount} {t("sessions can be restored for 24 hours.")}</small>
          </div>
          <button
            type="button"
            disabled={actionsLocked}
            onClick={() => void undoReapply()}
          >
            {t(undoingReapply ? "Restoring…" : "Undo reclassification")}
          </button>
        </div>
      )}
    </section>
  );
}
