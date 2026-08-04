import type {
  ActivityTag,
  ActivityTagInput,
  Project,
  ProjectInput,
} from "../../lib/ipc";

export type OrganizationItem = Project | ActivityTag;
export type OrganizationInput = ProjectInput | ActivityTagInput;

const HEX_COLOR = /^#[0-9A-F]{6}$/i;

export function organizationInput(
  name: string,
  color: string,
): OrganizationInput | null {
  const normalizedName = name.trim();
  const normalizedColor = color.toUpperCase();
  if (
    normalizedName.length === 0
    || [...normalizedName].length > 80
    || !HEX_COLOR.test(normalizedColor)
  ) {
    return null;
  }
  return { name: normalizedName, color: normalizedColor };
}

export function sortOrganizationItems<T extends OrganizationItem>(items: T[]): T[] {
  return [...items].sort((left, right) => (
    Number(left.archived) - Number(right.archived)
    || left.name.localeCompare(right.name)
    || left.id - right.id
  ));
}
