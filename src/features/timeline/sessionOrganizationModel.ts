import type { ActivityTag, Project } from "../../lib/ipc";
import { sortOrganizationItems } from "../settings/projectTagModel";

export function mergeOrganizationOptions<T extends Project | ActivityTag>(
  activeItems: T[],
  selectedItems: T[],
): T[] {
  const byId = new Map(activeItems.map((item) => [item.id, item]));
  for (const item of selectedItems) byId.set(item.id, item);
  return sortOrganizationItems([...byId.values()]);
}

export function toggleOrganizationId(ids: Set<number>, id: number): Set<number> {
  const next = new Set(ids);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

export function organizationSelectionChanged(
  initialProjectId: number | null,
  projectId: number | null,
  initialTagIds: Set<number>,
  tagIds: Set<number>,
): boolean {
  if (initialProjectId !== projectId || initialTagIds.size !== tagIds.size) return true;
  return [...initialTagIds].some((id) => !tagIds.has(id));
}

export function hasArchivedOrganizationSelection(
  projects: Project[],
  tags: ActivityTag[],
  projectId: number | null,
  tagIds: Set<number>,
): boolean {
  return projects.some((project) => project.archived && project.id === projectId)
    || tags.some((tag) => tag.archived && tagIds.has(tag.id));
}
