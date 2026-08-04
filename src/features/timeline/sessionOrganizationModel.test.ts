import { describe, expect, it } from "vitest";
import type { ActivityTag, Project } from "../../lib/ipc";
import {
  hasArchivedOrganizationSelection,
  mergeOrganizationOptions,
  organizationSelectionChanged,
  toggleOrganizationId,
} from "./sessionOrganizationModel";

function item(
  id: number,
  name: string,
  archived = false,
): Project | ActivityTag {
  return {
    id,
    name,
    color: "#527B69",
    archived,
    createdAtMs: id,
    updatedAtMs: id,
  };
}

describe("mergeOrganizationOptions", () => {
  it("keeps a selected archived item without duplicating active selections", () => {
    const active = [item(2, "Website"), item(1, "Client")];
    const selected = [item(2, "Website"), item(3, "Legacy", true)];

    expect(mergeOrganizationOptions(active, selected).map(({ id, archived }) => ({ id, archived })))
      .toEqual([
        { id: 1, archived: false },
        { id: 2, archived: false },
        { id: 3, archived: true },
      ]);
  });
});

describe("toggleOrganizationId", () => {
  it("adds and removes tag selections immutably", () => {
    const original = new Set([1]);
    expect([...toggleOrganizationId(original, 2)]).toEqual([1, 2]);
    expect([...toggleOrganizationId(original, 1)]).toEqual([]);
    expect([...original]).toEqual([1]);
  });
});

describe("organizationSelectionChanged", () => {
  it("compares tags without depending on selection order", () => {
    expect(organizationSelectionChanged(1, 1, new Set([2, 3]), new Set([3, 2]))).toBe(false);
    expect(organizationSelectionChanged(1, 2, new Set([2, 3]), new Set([2, 3]))).toBe(true);
    expect(organizationSelectionChanged(1, 1, new Set([2, 3]), new Set([2]))).toBe(true);
  });
});

describe("hasArchivedOrganizationSelection", () => {
  it("detects selected archived projects and tags", () => {
    const projects = [item(1, "Current", true)] as Project[];
    const tags = [item(2, "Legacy", true), item(3, "Current")] as ActivityTag[];

    expect(hasArchivedOrganizationSelection(projects, tags, 1, new Set())).toBe(true);
    expect(hasArchivedOrganizationSelection(projects, tags, null, new Set([2]))).toBe(true);
    expect(hasArchivedOrganizationSelection(projects, tags, null, new Set([3]))).toBe(false);
  });
});
