import { describe, expect, it } from "vitest";
import {
  categoryRuleActionsLocked,
  categoryRuleInputFromDraft,
} from "./CategoryRules";

describe("categoryRuleInputFromDraft", () => {
  it("normalizes valid values", () => {
    expect(categoryRuleInputFromDraft({
      matchField: "WINDOW_TITLE",
      pattern: "  github.com  ",
      category: "  Work  ",
      priority: "20",
      enabled: true,
    })).toEqual({
      matchField: "WINDOW_TITLE",
      pattern: "github.com",
      category: "Work",
      priority: 20,
      enabled: true,
    });
  });

  it("rejects empty and out-of-range values", () => {
    expect(categoryRuleInputFromDraft({
      matchField: "APPLICATION_NAME",
      pattern: "",
      category: "Work",
      priority: "100",
      enabled: true,
    })).toBeNull();
    expect(categoryRuleInputFromDraft({
      matchField: "BUNDLE_ID",
      pattern: "example",
      category: "Work",
      priority: "10000",
      enabled: true,
    })).toBeNull();
  });
});

describe("categoryRuleActionsLocked", () => {
  it("locks all mutations while a save or another mutation is in flight", () => {
    expect(categoryRuleActionsLocked(true, null, false)).toBe(true);
    expect(categoryRuleActionsLocked(false, 12, false)).toBe(true);
    expect(categoryRuleActionsLocked(false, null, true)).toBe(true);
    expect(categoryRuleActionsLocked(false, null, false)).toBe(false);
  });
});
