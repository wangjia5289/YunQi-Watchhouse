import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test("switches languages and opens the diagnostics center", async ({ page }) => {
  await page.goto("./?browser-mock");
  await expect(page.getByRole("navigation", { name: "Main navigation" })).toBeVisible();

  await page.getByRole("button", { name: "中文" }).click();
  await expect(page.getByRole("navigation", { name: "主导航" })).toBeVisible();
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(page.getByRole("heading", { name: "诊断中心" })).toBeVisible();
  await expect(page.getByText("数据库完整性")).toBeVisible();
});

test("opens reports and exposes weekly archive controls", async ({ page }) => {
  await page.goto("./?browser-mock");
  await page.getByRole("button", { name: "Reports", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Weekly insights" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Weekly report archive" })).toBeVisible();
  await page.getByRole("button", { name: "Archive this week" }).click();
  await expect(page.getByText("Weekly report archived locally.")).toBeVisible();
});

test("loads each top-level page on demand", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === "narrow", "lazy page loading runs once on desktop");
  await page.goto("./?browser-mock");

  const pages = [
    ["Timeline", "No activity recorded"],
    ["Search", "No matching activity"],
    ["Applications", "No application activity"],
    ["History", "Daily activity"],
    ["Reports", "Weekly insights"],
    ["Settings", "Diagnostics center"],
  ] as const;
  for (const [pageName, readyHeading] of pages) {
    await page.getByRole("button", { name: pageName, exact: true }).click();
    await expect(page.getByRole("heading", { name: pageName, exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: readyHeading, exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
  }
});

test("loads the isolated tray entry and controls tracking", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === "narrow", "tray entry smoke test runs once on desktop");
  await page.goto("./?browser-mock&window=tray-panel");

  const tray = page.locator("main.tray-panel");
  await expect(tray).toBeVisible();
  await expect(tray).toHaveCSS("border-radius", "12px");
  await expect(tray.getByText("Visual Studio Code", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Pause tracking", exact: true }).click();
  await expect(page.getByRole("button", { name: "Resume tracking", exact: true })).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("previews a classification rule before saving", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === "narrow", "classification editor workflow runs once on desktop");
  await page.goto("./?browser-mock");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("button", { name: "Add rule", exact: true }).click();
  await page.getByLabel("Contains", { exact: true }).fill("Visual Studio Code");
  await page.getByLabel("Category", { exact: true }).fill("Development");
  const preview = page.locator(".category-rule-preview");
  await expect(preview.getByText("Some matches are already handled by earlier rules.")).toBeVisible();
  await expect(preview.getByText("7", { exact: true })).toBeVisible();
  await expect(preview.getByText("6", { exact: true })).toBeVisible();
  await expect(preview.getByText("Earlier matching rules")).toBeVisible();
  await expect(preview.getByText("Handled earlier")).toBeVisible();
  await expect(preview.getByText("Visual Studio Code", { exact: true })).toBeVisible();
});

test("has no serious accessibility violations on primary views", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === "narrow", "accessibility scan runs once on desktop");
  await page.goto("./?browser-mock");
  const todayResults = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa"])
    .disableRules(["color-contrast"])
    .analyze();
  expect(todayResults.violations.filter((item) => ["serious", "critical"].includes(item.impact ?? ""))).toEqual([]);

  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Diagnostics center" })).toBeVisible();
  const settingsResults = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa"])
    .disableRules(["color-contrast"])
    .analyze();
  expect(settingsResults.violations.filter((item) => ["serious", "critical"].includes(item.impact ?? ""))).toEqual([]);
});

test("keeps the application inside a narrow viewport", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "narrow", "narrow layout is covered by the narrow project");
  await page.goto("./?browser-mock");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Diagnostics center" })).toBeVisible();
  const overflow = await page.evaluate(() => ({
    document: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    body: document.body.scrollWidth - document.body.clientWidth,
  }));
  expect(overflow.document).toBeLessThanOrEqual(1);
  expect(overflow.body).toBeLessThanOrEqual(1);
});
