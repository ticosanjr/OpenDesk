import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("onboarding dialog reports real workspace readiness", async ({ page, request }) => {
  const workspace = await seedWorkspace(request, "Onboarding");
  await page.goto("/#/");

  await page.getByRole("button", { name: /Primeiros passos/ }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("Essenciais", { exact: true })).toBeVisible();
  await expect(dialog.getByText("Workspace ativo")).toBeVisible();
  await expect(dialog.getByText(workspace.root, { exact: false })).toBeVisible();
  await expect(dialog.getByRole("button", { name: /Verificar/ })).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});
