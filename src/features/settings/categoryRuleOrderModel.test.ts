import { describe, expect, it } from "vitest";
import { moveCategoryRuleId, offsetCategoryRuleId } from "./categoryRuleOrderModel";

describe("category rule ordering", () => {
  it("moves a dragged rule to the target position without mutating the input", () => {
    const original = [1, 2, 3, 4];
    expect(moveCategoryRuleId(original, 1, 3)).toEqual([2, 3, 1, 4]);
    expect(moveCategoryRuleId(original, 4, 2)).toEqual([1, 4, 2, 3]);
    expect(original).toEqual([1, 2, 3, 4]);
  });

  it("moves one position and leaves boundary requests unchanged", () => {
    expect(offsetCategoryRuleId([1, 2, 3], 2, -1)).toEqual([2, 1, 3]);
    expect(offsetCategoryRuleId([1, 2, 3], 2, 1)).toEqual([1, 3, 2]);
    expect(offsetCategoryRuleId([1, 2, 3], 1, -1)).toEqual([1, 2, 3]);
    expect(offsetCategoryRuleId([1, 2, 3], 3, 1)).toEqual([1, 2, 3]);
  });
});
