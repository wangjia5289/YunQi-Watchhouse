import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import ts from "typescript";
import { formatDuration } from "./format";
import { translateText } from "./i18n";

describe("translateText", () => {
  it("translates exact interface copy while preserving whitespace", () => {
    expect(translateText("  Timeline ")).toBe("  时间线 ");
    expect(translateText("Settings")).toBe("设置");
  });

  it("translates dynamic count patterns", () => {
    expect(translateText("12 sessions")).toBe("12 个会话");
    expect(translateText("3 selected")).toBe("已选择 3 项");
    expect(translateText("8 active time blocks")).toBe("8 个活跃时间块");
    expect(translateText("2h remaining")).toBe("剩余 2 小时");
    expect(translateText("Select Safari session")).toBe("选择“Safari”会话");
    expect(translateText("2 active projects")).toBe("2 个启用项目");
    expect(translateText("3 active activity tags")).toBe("3 个启用标签");
    expect(translateText("Edit project Client launch")).toBe("编辑项目“Client launch”");
    expect(translateText("tag Deep work active")).toBe("标签“Deep work”启用状态");
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
      "Weekly report archive",
      "Encrypted backup",
      "Diagnostics center",
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

  it("translates errors from encrypted backups and weekly notifications", () => {
    expect(translateText("The encrypted backup file could not be written.")).toBe(
      "无法写入加密备份文件。",
    );
    expect(translateText("notification permission has not been granted")).toBe(
      "尚未获得通知权限。",
    );
    expect(translateText("sessions assigned to different projects cannot be merged")).toBe(
      "已归属不同项目的会话无法合并。",
    );
  });

  it("formats durations compactly in every interface language", () => {
    expect(formatDuration(135 * 60_000, "zh-CN")).toBe("2h 15min");
    expect(formatDuration(135 * 60_000, "en")).toBe("2h 15min");
    expect(formatDuration(45 * 60_000, "zh-CN")).toBe("45min");
    expect(formatDuration(2 * 60 * 60_000, "en")).toBe("2h");
  });

  it("keeps interface source free of raw English JSX copy", () => {
    const root = new URL("../", import.meta.url);
    const files = readdirSync(root, { recursive: true })
      .filter((file): file is string => typeof file === "string" && file.endsWith(".tsx"));
    for (const file of files) {
      const source = readFileSync(new URL(file, root), "utf8");
      const sourceFile = ts.createSourceFile(
        file,
        source,
        ts.ScriptTarget.Latest,
        true,
        ts.ScriptKind.TSX,
      );
      const rawText: string[] = [];
      const visit = (node: ts.Node) => {
        if (ts.isJsxText(node)) {
          const value = node.text.trim();
          if (/[A-Za-z]/.test(value) && value !== "EN") rawText.push(value);
        }
        if (
          ts.isJsxAttribute(node)
          && ["aria-label", "placeholder", "title"].includes(node.name.getText())
          && node.initializer
          && ts.isStringLiteral(node.initializer)
          && /[A-Za-z]/.test(node.initializer.text)
        ) {
          rawText.push(`${node.name.getText()}="${node.initializer.text}"`);
        }
        ts.forEachChild(node, visit);
      };
      visit(sourceFile);
      expect(rawText, file).toEqual([]);
    }
  });

  it("defines translations for literal copy passed to t", () => {
    const root = new URL("../", import.meta.url);
    const files = readdirSync(root, { recursive: true })
      .filter((file): file is string => typeof file === "string" && file.endsWith(".tsx"));
    const intentionalFallbacks = new Set(["English", "Watchhouse", "YunQi-Watchhouse"]);
    const untranslated: string[] = [];
    for (const file of files) {
      const source = readFileSync(new URL(file, root), "utf8");
      const sourceFile = ts.createSourceFile(
        file,
        source,
        ts.ScriptTarget.Latest,
        true,
        ts.ScriptKind.TSX,
      );
      const visit = (node: ts.Node) => {
        if (
          ts.isCallExpression(node)
          && ts.isIdentifier(node.expression)
          && node.expression.text === "t"
          && node.arguments[0]
        ) {
          const inspectArgument = (argumentNode: ts.Node) => {
            if (
              ts.isStringLiteralLike(argumentNode)
              && /[A-Za-z]/.test(argumentNode.text)
              && !intentionalFallbacks.has(argumentNode.text)
              && translateText(argumentNode.text) === argumentNode.text
            ) {
              untranslated.push(`${file}: ${argumentNode.text}`);
            }
            if (ts.isParenthesizedExpression(argumentNode)) {
              inspectArgument(argumentNode.expression);
            } else if (ts.isConditionalExpression(argumentNode)) {
              inspectArgument(argumentNode.whenTrue);
              inspectArgument(argumentNode.whenFalse);
            }
          };
          inspectArgument(node.arguments[0]);
        }
        ts.forEachChild(node, visit);
      };
      visit(sourceFile);
    }
    expect(untranslated).toEqual([]);
  });
});
