import { describe, expect, it } from "vitest";
import { CategoryRulePreview } from "../../lib/ipc";
import { categoryRulePreviewState } from "./categoryRulePreviewModel";

function preview(overrides: Partial<CategoryRulePreview> = {}): CategoryRulePreview {
  return {
    matchedSessionCount: 4,
    matchedApplicationCount: 2,
    effectiveSessionCount: 4,
    shadowedSessionCount: 0,
    conflicts: [],
    samples: [],
    ...overrides,
  };
}

describe("categoryRulePreviewState", () => {
  it("distinguishes empty and disabled drafts", () => {
    expect(categoryRulePreviewState(preview({ matchedSessionCount: 0 }), true))
      .toBe("NO_MATCHES");
    expect(categoryRulePreviewState(preview(), false)).toBe("DISABLED");
  });

  it("distinguishes fully and partially shadowed matches", () => {
    expect(categoryRulePreviewState(preview({
      effectiveSessionCount: 0,
      shadowedSessionCount: 4,
    }), true)).toBe("FULLY_SHADOWED");
    expect(categoryRulePreviewState(preview({
      effectiveSessionCount: 2,
      shadowedSessionCount: 2,
    }), true)).toBe("PARTIALLY_SHADOWED");
    expect(categoryRulePreviewState(preview(), true)).toBe("WILL_APPLY");
  });
});
