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

  it("covers every major application surface", () => {
    const requiredCopy = [
      "Recorded locally on this Mac",
      "Ignore future activity",
      "No history yet",
      "Plan history",
      "Notification Permission",
      "Local storage",
      "Retention and backups",
      "Data Health",
      "View Privacy Notice",
      "Restore Database Backup",
      "Delete All Activity Data",
      "Import Watchhouse data",
      "These sessions will be removed from the timeline. You can undo this operation afterward.",
    ];
    for (const copy of requiredCopy) {
      expect(translateText(copy), copy).not.toBe(copy);
    }
  });

  it("covers dynamic operation and focus messages", () => {
    expect(translateText("Imported 12; merged 2; skipped 1.")).toBe(
      "已导入 12 个，合并 2 个，跳过 1 个。",
    );
    expect(translateText("45-minute focus plan started.")).toBe(
      "已开始 45 分钟专注计划。",
    );
    expect(translateText("Backup saved to /tmp/watchhouse.sqlite3")).toBe(
      "备份已保存到 /tmp/watchhouse.sqlite3",
    );
  });

  it("translates native confirmation copy", () => {
    expect(translateText(
      "Delete all recorded activity? This cannot be undone.",
    )).toBe("要删除所有活动记录吗？此操作无法撤销。");
  });
});
