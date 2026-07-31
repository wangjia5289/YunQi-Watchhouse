import { describe, expect, it } from "vitest";
import { translateText } from "./i18n";

describe("translateText", () => {
  it("translates exact interface copy while preserving whitespace", () => {
    expect(translateText("  Timeline ")).toBe("  时间线 ");
    expect(translateText("Settings")).toBe("设置");
  });

  it("translates dynamic count patterns", () => {
    expect(translateText("12 sessions")).toBe("12 个会话");
    expect(translateText("3 selected")).toBe("已选择 3 项");
    expect(translateText("Show more sessions (200 of 450)")).toBe(
      "显示更多会话（200/450）",
    );
  });

  it("keeps unknown copy as the English fallback", () => {
    expect(translateText("Future feature")).toBe("Future feature");
  });
});
