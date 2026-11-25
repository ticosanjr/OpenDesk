import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("consolidated tabs and legacy redirects stay deep-linkable", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Navigation Tabs");

  await page.goto("/#/runs");
  await page.getByRole("tab", { name: "Worktrees" }).click();
  await expect(page).toHaveURL(/tab=worktrees/);
  await expect(page.getByRole("tab", { name: /Isolados/ })).toBeVisible();

  await page.goto("/#/worktrees");
  await expect(page).toHaveURL(/\/runs\?tab=worktrees/);

  await page.goto("/#/search");
  await page.getByRole("tab", { name: "Arquivos" }).click();
  await expect(page).toHaveURL(/tab=files/);
  await expect(
    page.getByRole("button", { name: "Todo o workspace" }),
  ).toBeVisible();

  await page.goto("/#/files");
  await expect(page).toHaveURL(/\/search\?tab=files/);

  await page.goto("/#/chat");
  await page.getByRole("tab", { name: "Criador" }).click();
  await expect(page).toHaveURL(/tab=builder/);
  await expect(page.getByText("Meta-Builder").first()).toBeVisible();

  await page.goto("/#/chat?tab=builder");
  await expect(page.getByText("Meta-Builder").first()).toBeVisible();
});
