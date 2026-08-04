import { describe, expect, it } from "vitest";
import type { Project } from "../../lib/ipc";
import { organizationInput, sortOrganizationItems } from "./projectTagModel";

function project(id: number, name: string, archived = false): Project {
  return {
    id,
    name,
    color: "#507A68",
    archived,
    createdAtMs: id,
    updatedAtMs: id,
  };
}

describe("organizationInput", () => {
  it("trims names and normalizes colors", () => {
    expect(organizationInput("  Client launch  ", "#5a7fc8")).toEqual({
      name: "Client launch",
      color: "#5A7FC8",
    });
  });

  it("rejects blank, oversized, and malformed values", () => {
    expect(organizationInput("  ", "#5A7FC8")).toBeNull();
    expect(organizationInput("a".repeat(81), "#5A7FC8")).toBeNull();
    expect(organizationInput("Client", "blue")).toBeNull();
  });
});

describe("sortOrganizationItems", () => {
  it("keeps active items before archived items and sorts each group by name", () => {
    expect(sortOrganizationItems([
      project(3, "Archived", true),
      project(2, "Zulu"),
      project(1, "Alpha"),
    ]).map((item) => item.id)).toEqual([1, 2, 3]);
  });
});
