import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test("switches languages and opens the diagnostics center", async ({ page }) => {
  await page.goto("./?browser-mock");
  await expect(page.getByRole("navigation", { name: "Main navigation" })).toBeVisible();

  await page.getByRole("button", { name: "中文" }).click();
  await expect(page.getByRole("navigation", { name: "主导航" })).toBeVisible();
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("tab", { name: "诊断与更新", exact: true }).click();
  await expect(page.getByRole("heading", { name: "诊断中心" })).toBeVisible();
  await expect(page.getByText("数据库完整性")).toBeVisible();
});

test("opens reports and exposes weekly archive controls", async ({ page }) => {
  await page.goto("./?browser-mock");
  await page.getByRole("button", { name: "Reports", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Weekly insights" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Weekly report archive" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Projects & activity tags" })).toBeVisible();
  await expect(page.getByText("Client launch", { exact: true })).toBeVisible();
  await expect(page.getByText("Tags may overlap; durations do not add up.", { exact: true }))
    .toBeVisible();
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
    ["Settings", "Application"],
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
  await page.getByRole("tab", { name: "Classification", exact: true }).click();
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
  await expect(page.getByRole("heading", { name: "Application", exact: true })).toBeVisible();
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
  await page.getByRole("tab", { name: "Diagnostics & Updates", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Diagnostics center" })).toBeVisible();
  const overflow = await page.evaluate(() => ({
    document: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    body: document.body.scrollWidth - document.body.clientWidth,
  }));
  expect(overflow.document).toBeLessThanOrEqual(1);
  expect(overflow.body).toBeLessThanOrEqual(1);
});

test("mounts one settings tab on demand", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === "narrow", "settings tab workflow runs once on desktop");
  await page.goto("./?browser-mock");
  await page.getByRole("button", { name: "Settings", exact: true }).click();

  const tabs = page.getByRole("tablist", { name: "Settings sections" });
  await expect(tabs).toBeVisible();
  await expect(page.getByRole("tab", { name: "General", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("heading", { name: "Application", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Automatic classification", exact: true })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Diagnostics center", exact: true })).toHaveCount(0);

  await page.getByRole("tab", { name: "Classification", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Automatic classification", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Application", exact: true })).toHaveCount(0);

  await page.getByRole("tab", { name: "Focus & Limits", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Goals and breaks", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Application usage limits", exact: true })).toBeVisible();

  await page.getByRole("tab", { name: "Data & Safety", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Local storage", exact: true })).toBeVisible();

  await page.getByRole("tab", { name: "Diagnostics & Updates", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Software updates", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Diagnostics center", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Local storage", exact: true })).toHaveCount(0);
});

test("creates, edits, archives, and restores a project", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === "narrow", "project manager workflow runs once on desktop");
  await page.goto("./?browser-mock");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("tab", { name: "Classification", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Projects & activity tags", exact: true })).toBeVisible();
  const manager = page.locator(".project-tag-manager");

  await page.getByLabel("New project name", { exact: true }).fill("client launch");
  await page.getByRole("button", { name: "Add project", exact: true }).click();
  await expect(manager.getByRole("alert")).toHaveText("a project with this name already exists");
  await expect(manager.getByRole("status")).toHaveCount(0);

  await page.getByLabel("New project name", { exact: true }).fill("Website refresh");
  await page.getByLabel("New project color", { exact: true }).fill("#39796a");
  await page.getByRole("button", { name: "Add project", exact: true }).click();
  await expect(page.getByText("Website refresh", { exact: true })).toBeVisible();
  await expect(manager.getByRole("alert")).toHaveCount(0);
  await expect(manager.getByRole("status")).toHaveText("Project added.");

  await page.getByRole("button", { name: "Edit project Website refresh", exact: true }).click();
  await page.getByLabel("Edit project name", { exact: true }).fill("Website relaunch");
  await page.getByRole("button", { name: "Save project", exact: true }).click();
  await expect(page.getByText("Website relaunch", { exact: true })).toBeVisible();

  await page.getByRole("switch", { name: "project Website relaunch active", exact: true }).click();
  await expect(page.getByText("Website relaunch", { exact: true })).toHaveCount(0);
  await page.getByLabel("Show archived", { exact: true }).check();
  await expect(page.getByText("Website relaunch", { exact: true })).toBeVisible();
  await page.getByRole("switch", { name: "project Website relaunch active", exact: true }).click();
  await expect(page.getByRole("switch", { name: "project Website relaunch active", exact: true }))
    .toHaveAttribute("aria-checked", "true");

  await page.getByRole("button", { name: "中文", exact: true }).click();
  await expect(manager.getByText("2 个启用项目", { exact: true })).toBeVisible();
  await expect(manager.getByRole("button", { name: "编辑项目“Client launch”", exact: true }))
    .toBeVisible();
  await expect(manager.getByRole("switch", { name: "项目“Client launch”启用状态", exact: true }))
    .toBeVisible();
});

test("edits a session project and tags on demand", async ({ page }) => {
  await page.goto("./?browser-mock&organization-flow");
  await page.getByRole("button", { name: "Timeline", exact: true }).click();
  await page.getByRole("button", { name: "Details", exact: true }).click();
  const editButton = page.getByRole("button", {
    name: "Edit Visual Studio Code session",
    exact: true,
  });
  await editButton.click();

  const dialog = page.getByRole("dialog", { name: "Edit recorded session" });
  await expect(dialog.getByLabel("Start", { exact: true })).toBeFocused();
  await expect(dialog.getByRole("combobox", { name: "Project", exact: true })).toBeVisible();
  await dialog.getByRole("combobox", { name: "Project", exact: true }).selectOption({ label: "Client launch" });
  await dialog.getByRole("checkbox", { name: "Deep work", exact: true }).check();
  await dialog.getByRole("button", { name: "Save changes", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(editButton).toBeFocused();

  await editButton.click();
  const reopened = page.getByRole("dialog", { name: "Edit recorded session" });
  await expect(reopened.getByLabel("Start", { exact: true })).toBeFocused();
  await expect(reopened.getByRole("combobox", { name: "Project", exact: true })).toHaveValue("1");
  await expect(reopened.getByRole("checkbox", { name: "Deep work", exact: true })).toBeChecked();
  await page.keyboard.press("Escape");
  await expect(reopened).toHaveCount(0);
  await expect(editButton).toBeFocused();

  await expect(page.getByText("Client launch", { exact: true })).toBeVisible();
  await expect(page.getByText("Deep work", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Advanced", exact: true }).click();
  await page.getByRole("combobox", { name: "Project", exact: true })
    .selectOption({ label: "Client launch" });
  const organizationBadges = page.locator(".session-organization-badges");
  await expect(page.getByText("Visual Studio Code", { exact: true })).toBeVisible();

  await page.getByRole("checkbox", { name: "Select Visual Studio Code session", exact: true })
    .check();
  const organizeButton = page.getByRole("button", { name: "Organize", exact: true });
  await organizeButton.click();
  let bulkDialog = page.getByRole("dialog", { name: "Organize selected sessions" });
  await expect(bulkDialog.getByRole("combobox", { name: "Project", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(bulkDialog).toHaveCount(0);
  await expect(organizeButton).toBeFocused();

  await organizeButton.click();
  bulkDialog = page.getByRole("dialog", { name: "Organize selected sessions" });
  await bulkDialog.getByRole("combobox", { name: "Project", exact: true })
    .selectOption({ label: "Client launch" });
  await bulkDialog.getByRole("checkbox", { name: "Review", exact: true }).check();
  await bulkDialog.getByRole("button", { name: "Apply changes", exact: true }).click();
  await expect(bulkDialog).toHaveCount(0);
  await expect(organizationBadges.getByText("Review", { exact: true })).toBeVisible();
  await expect(organizationBadges.getByText("Deep work", { exact: true })).toHaveCount(0);

  await page.getByRole("button", { name: "Undo (1)", exact: true }).click();
  await expect(organizationBadges.getByText("Client launch", { exact: true })).toBeVisible();
  await expect(organizationBadges.getByText("Deep work", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Search", exact: true }).click();
  const searchResults = page.getByRole("region", { name: "Search results", exact: true });
  const searchBadges = searchResults.locator(".session-organization-badges");
  await expect(searchBadges.getByText("Client launch", { exact: true })).toBeVisible();
  await expect(searchBadges.getByText("Deep work", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Advanced", exact: true }).click();
  const searchFilters = page.getByRole("region", { name: "Search filters", exact: true });
  const projectFilter = searchFilters.getByRole("combobox", { name: "Project", exact: true });
  const tagFilter = searchFilters.getByRole("combobox", { name: "Activity tag", exact: true });
  await projectFilter.selectOption({ label: "Client launch" });
  await tagFilter.selectOption({ label: "Deep work" });
  await expect(searchBadges.getByText("Deep work", { exact: true })).toBeVisible();

  await tagFilter.selectOption({ label: "Review" });
  await expect(searchResults.getByRole("heading", { name: "No matching activity" })).toBeVisible();
  await tagFilter.selectOption({ label: "Deep work" });
  await expect(searchBadges.getByText("Deep work", { exact: true })).toBeVisible();

  await searchFilters.getByRole("checkbox", { name: "Unassigned only", exact: true }).check();
  await expect(projectFilter).toBeDisabled();
  await expect(tagFilter).toBeDisabled();
  await expect(searchResults.getByRole("heading", { name: "No matching activity" })).toBeVisible();
});

test("restores compatible saved searches and rejects contradictory organization filters", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name === "narrow", "saved-search compatibility runs once on desktop");
  await page.addInitScript(() => {
    const base = {
      preset: "7_DAYS",
      startDate: "2026-07-30",
      endDate: "2026-08-05",
      query: "",
      stateFilter: "ALL",
      minimumMinutes: "",
      maximumMinutes: "",
      timeFrom: "",
      timeTo: "",
    };
    localStorage.setItem("watchhouse.globalSearch.saved.v1", JSON.stringify([
      { ...base, id: "legacy", name: "Legacy search" },
      {
        ...base,
        id: "organized",
        name: "Organized work",
        projectId: 1,
        tagId: 10,
        unassignedOnly: false,
      },
      {
        ...base,
        id: "contradictory",
        name: "Contradictory search",
        projectId: 1,
        unassignedOnly: true,
      },
    ]));
  });

  await page.goto("./?browser-mock&organization-flow");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  const savedSearches = page.getByRole("region", { name: "Saved searches", exact: true });
  await expect(savedSearches.getByRole("button", { name: "Legacy search", exact: true }))
    .toBeVisible();
  await expect(savedSearches.getByRole("button", { name: "Contradictory search", exact: true }))
    .toHaveCount(0);

  await savedSearches.getByRole("button", { name: "Organized work", exact: true }).click();
  const filters = page.getByRole("region", { name: "Search filters", exact: true });
  await expect(filters.getByRole("combobox", { name: "Project", exact: true })).toHaveValue("1");
  await expect(filters.getByRole("combobox", { name: "Activity tag", exact: true }))
    .toHaveValue("10");
  await expect(filters.getByRole("checkbox", { name: "Unassigned only", exact: true }))
    .not.toBeChecked();
});
