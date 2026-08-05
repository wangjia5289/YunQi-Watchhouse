import { FormEvent, useEffect, useState } from "react";
import {
  ActivityTag,
  createActivityTag,
  createProject,
  errorMessage,
  listActivityTags,
  listProjects,
  Project,
  setActivityTagArchived,
  setProjectArchived,
  updateActivityTag,
  updateProject,
} from "../../lib/ipc";
import { useLocale } from "../../lib/i18n";
import {
  organizationInput,
  OrganizationItem,
  sortOrganizationItems,
} from "./projectTagModel";
import "./ProjectTagManager.css";

type ItemKind = "project" | "tag";

interface Draft {
  name: string;
  color: string;
}

interface EditingDraft extends Draft {
  kind: ItemKind;
  id: number;
}

type OperationFeedback = {
  kind: "success" | "error";
  message: string;
};

const EMPTY_PROJECT: Draft = { name: "", color: "#507A68" };
const EMPTY_TAG: Draft = { name: "", color: "#6A71B8" };

function replaceItem<T extends OrganizationItem>(items: T[], updated: T): T[] {
  return sortOrganizationItems(items.map((item) => item.id === updated.id ? updated : item));
}

function EditIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4z" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="m5 12 4 4L19 6" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="m6 6 12 12M18 6 6 18" />
    </svg>
  );
}

interface ManagerColumnProps {
  kind: ItemKind;
  items: OrganizationItem[];
  draft: Draft;
  editing: EditingDraft | null;
  disabled: boolean;
  showArchived: boolean;
  onDraftChange: (draft: Draft) => void;
  onCreate: (event: FormEvent<HTMLFormElement>) => void;
  onEdit: (item: OrganizationItem) => void;
  onEditChange: (draft: EditingDraft) => void;
  onSaveEdit: () => void;
  onCancelEdit: () => void;
  onArchivedChange: (item: OrganizationItem, archived: boolean) => void;
}

function ManagerColumn({
  kind,
  items,
  draft,
  editing,
  disabled,
  showArchived,
  onDraftChange,
  onCreate,
  onEdit,
  onEditChange,
  onSaveEdit,
  onCancelEdit,
  onArchivedChange,
}: ManagerColumnProps) {
  const { t } = useLocale();
  const singular = kind === "project" ? "project" : "tag";
  const heading = kind === "project" ? "Projects" : "Activity tags";
  const activeCount = items.filter((item) => !item.archived).length;
  const visibleItems = items.filter((item) => showArchived || !item.archived);

  return (
    <section className="project-tag-group" aria-labelledby={`${kind}-manager-title`}>
      <div className="project-tag-group-heading">
        <div>
          <h3 id={`${kind}-manager-title`}>{t(heading)}</h3>
          <span>{t(kind === "project"
            ? `${activeCount} active projects`
            : `${activeCount} active activity tags`)}</span>
        </div>
      </div>

      <form className="project-tag-create" onSubmit={onCreate}>
        <input
          type="color"
          value={draft.color}
          disabled={disabled}
          aria-label={t(kind === "project" ? "New project color" : "New tag color")}
          title={t(kind === "project" ? "New project color" : "New tag color")}
          onChange={(event) => onDraftChange({ ...draft, color: event.currentTarget.value })}
        />
        <input
          type="text"
          value={draft.name}
          maxLength={80}
          disabled={disabled}
          aria-label={t(kind === "project" ? "New project name" : "New tag name")}
          placeholder={t(kind === "project" ? "New project" : "New tag")}
          onChange={(event) => onDraftChange({ ...draft, name: event.currentTarget.value })}
        />
        <button
          type="submit"
          className="project-tag-icon-button primary"
          disabled={disabled || organizationInput(draft.name, draft.color) === null}
          aria-label={t(kind === "project" ? "Add project" : "Add tag")}
          title={t(kind === "project" ? "Add project" : "Add tag")}
        >
          <PlusIcon />
        </button>
      </form>

      {visibleItems.length === 0 ? (
        <p className="project-tag-empty">
          {t(kind === "project" ? "No projects yet." : "No activity tags yet.")}
        </p>
      ) : (
        <div className="project-tag-list">
          {visibleItems.map((item) => {
            const isEditing = editing?.kind === kind && editing.id === item.id;
            return (
              <article className={`project-tag-item${item.archived ? " archived" : ""}`} key={item.id}>
                {isEditing && editing ? (
                  <>
                    <input
                      type="color"
                      className="project-tag-color-input"
                      value={editing.color}
                      disabled={disabled}
                      aria-label={t(kind === "project" ? "Edit project color" : "Edit tag color")}
                      title={t(kind === "project" ? "Edit project color" : "Edit tag color")}
                      onChange={(event) => onEditChange({
                        ...editing,
                        color: event.currentTarget.value,
                      })}
                    />
                    <input
                      className="project-tag-name-input"
                      value={editing.name}
                      maxLength={80}
                      disabled={disabled}
                      aria-label={t(kind === "project" ? "Edit project name" : "Edit tag name")}
                      onChange={(event) => onEditChange({
                        ...editing,
                        name: event.currentTarget.value,
                      })}
                    />
                    <div className="project-tag-item-actions">
                      <button
                        type="button"
                        className="project-tag-icon-button primary"
                        disabled={disabled || organizationInput(editing.name, editing.color) === null}
                        aria-label={t(kind === "project" ? "Save project" : "Save tag")}
                        title={t(kind === "project" ? "Save project" : "Save tag")}
                        onClick={onSaveEdit}
                      >
                        <CheckIcon />
                      </button>
                      <button
                        type="button"
                        className="project-tag-icon-button"
                        disabled={disabled}
                        aria-label={t("Cancel")}
                        title={t("Cancel")}
                        onClick={onCancelEdit}
                      >
                        <CloseIcon />
                      </button>
                    </div>
                  </>
                ) : (
                  <>
                    <span
                      className="project-tag-swatch"
                      style={{ backgroundColor: item.color }}
                      aria-hidden="true"
                    />
                    <div className="project-tag-copy">
                      <strong>{item.name}</strong>
                      {item.archived && <small>{t("Archived")}</small>}
                    </div>
                    <div className="project-tag-item-actions">
                      <button
                        type="button"
                        className="project-tag-icon-button"
                        disabled={disabled}
                        aria-label={t(`Edit ${singular} ${item.name}`)}
                        title={t(kind === "project" ? "Edit project" : "Edit tag")}
                        onClick={() => onEdit(item)}
                      >
                        <EditIcon />
                      </button>
                      <button
                        type="button"
                        role="switch"
                        aria-checked={!item.archived}
                        className={`project-tag-active-toggle${item.archived ? "" : " active"}`}
                        disabled={disabled}
                        aria-label={t(`${singular} ${item.name} active`)}
                        title={t(kind === "project"
                          ? (item.archived ? "Restore project" : "Archive project")
                          : (item.archived ? "Restore tag" : "Archive tag"))}
                        onClick={() => onArchivedChange(item, !item.archived)}
                      >
                        <span />
                      </button>
                    </div>
                  </>
                )}
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}

export function ProjectTagManager() {
  const { t } = useLocale();
  const [projects, setProjects] = useState<Project[]>([]);
  const [tags, setTags] = useState<ActivityTag[]>([]);
  const [projectDraft, setProjectDraft] = useState<Draft>(EMPTY_PROJECT);
  const [tagDraft, setTagDraft] = useState<Draft>(EMPTY_TAG);
  const [editing, setEditing] = useState<EditingDraft | null>(null);
  const [showArchived, setShowArchived] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [operationFeedback, setOperationFeedback] = useState<OperationFeedback | null>(null);

  async function loadItems() {
    setLoading(true);
    setLoadError(null);
    setOperationFeedback(null);
    try {
      const [nextProjects, nextTags] = await Promise.all([
        listProjects(true),
        listActivityTags(true),
      ]);
      setProjects(sortOrganizationItems(nextProjects));
      setTags(sortOrganizationItems(nextTags));
    } catch (reason) {
      setLoadError(errorMessage(reason));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadItems();
  }, []);

  async function createItem(kind: ItemKind, draft: Draft) {
    const input = organizationInput(draft.name, draft.color);
    if (!input) return;
    setBusy(true);
    setOperationFeedback(null);
    try {
      if (kind === "project") {
        const created = await createProject(input);
        setProjects((current) => sortOrganizationItems([...current, created]));
        setProjectDraft(EMPTY_PROJECT);
      } else {
        const created = await createActivityTag(input);
        setTags((current) => sortOrganizationItems([...current, created]));
        setTagDraft(EMPTY_TAG);
      }
      setOperationFeedback({
        kind: "success",
        message: kind === "project" ? "Project added." : "Activity tag added.",
      });
    } catch (reason) {
      setOperationFeedback({ kind: "error", message: errorMessage(reason) });
    } finally {
      setBusy(false);
    }
  }

  async function saveEdit() {
    if (!editing) return;
    const input = organizationInput(editing.name, editing.color);
    if (!input) return;
    setBusy(true);
    setOperationFeedback(null);
    try {
      if (editing.kind === "project") {
        const updated = await updateProject(editing.id, input);
        setProjects((current) => replaceItem(current, updated));
      } else {
        const updated = await updateActivityTag(editing.id, input);
        setTags((current) => replaceItem(current, updated));
      }
      setEditing(null);
      setOperationFeedback({
        kind: "success",
        message: editing.kind === "project" ? "Project updated." : "Activity tag updated.",
      });
    } catch (reason) {
      setOperationFeedback({ kind: "error", message: errorMessage(reason) });
    } finally {
      setBusy(false);
    }
  }

  async function setArchived(kind: ItemKind, item: OrganizationItem, archived: boolean) {
    setBusy(true);
    setOperationFeedback(null);
    try {
      if (kind === "project") {
        const updated = await setProjectArchived(item.id, archived);
        setProjects((current) => replaceItem(current, updated));
      } else {
        const updated = await setActivityTagArchived(item.id, archived);
        setTags((current) => replaceItem(current, updated));
      }
      if (editing?.kind === kind && editing.id === item.id) setEditing(null);
      setOperationFeedback({
        kind: "success",
        message: archived
          ? (kind === "project" ? "Project archived." : "Activity tag archived.")
          : (kind === "project" ? "Project restored." : "Activity tag restored."),
      });
    } catch (reason) {
      setOperationFeedback({ kind: "error", message: errorMessage(reason) });
    } finally {
      setBusy(false);
    }
  }

  const actionsDisabled = loading || loadError !== null || busy;

  return (
    <section className="settings-card project-tag-manager">
      <div className="list-heading project-tag-manager-heading">
        <div>
          <p className="section-kicker">{t("Organization")}</p>
          <h2>{t("Projects & activity tags")}</h2>
        </div>
        <label className="project-tag-archive-filter">
          <input
            type="checkbox"
            checked={showArchived}
            onChange={(event) => setShowArchived(event.currentTarget.checked)}
          />
          <span>{t("Show archived")}</span>
        </label>
      </div>
      <p className="settings-note">
        {t("Group recorded sessions into projects and reusable tags.")}
      </p>

      {loadError && !loading && (
        <div className="project-tag-error" role="alert">
          <span>{t(loadError)}</span>
          <button type="button" onClick={() => void loadItems()}>{t("Retry")}</button>
        </div>
      )}
      {operationFeedback?.kind === "success" && (
        <p className="project-tag-notice" role="status">{t(operationFeedback.message)}</p>
      )}
      {operationFeedback?.kind === "error" && (
        <p className="project-tag-error" role="alert">{t(operationFeedback.message)}</p>
      )}

      {loading ? (
        <p className="project-tag-loading">{t("Loading projects and tags…")}</p>
      ) : loadError ? null : (
        <div className="project-tag-columns">
          <ManagerColumn
            kind="project"
            items={projects}
            draft={projectDraft}
            editing={editing}
            disabled={actionsDisabled}
            showArchived={showArchived}
            onDraftChange={setProjectDraft}
            onCreate={(event) => {
              event.preventDefault();
              void createItem("project", projectDraft);
            }}
            onEdit={(item) => setEditing({
              kind: "project",
              id: item.id,
              name: item.name,
              color: item.color,
            })}
            onEditChange={setEditing}
            onSaveEdit={() => void saveEdit()}
            onCancelEdit={() => setEditing(null)}
            onArchivedChange={(item, archived) => void setArchived("project", item, archived)}
          />
          <ManagerColumn
            kind="tag"
            items={tags}
            draft={tagDraft}
            editing={editing}
            disabled={actionsDisabled}
            showArchived={showArchived}
            onDraftChange={setTagDraft}
            onCreate={(event) => {
              event.preventDefault();
              void createItem("tag", tagDraft);
            }}
            onEdit={(item) => setEditing({
              kind: "tag",
              id: item.id,
              name: item.name,
              color: item.color,
            })}
            onEditChange={setEditing}
            onSaveEdit={() => void saveEdit()}
            onCancelEdit={() => setEditing(null)}
            onArchivedChange={(item, archived) => void setArchived("tag", item, archived)}
          />
        </div>
      )}
    </section>
  );
}
