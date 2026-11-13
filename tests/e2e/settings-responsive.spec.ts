import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("settings rails switch to a select below the desktop breakpoint", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Settings Responsive");
  await page.setViewportSize({ width: 900, height: 800 });
  await page.goto("/#/settings");
  const settingsSelect = page.getByRole("combobox", {
    name: "Seções de configuração",
  });
  await expect(settingsSelect).toBeVisible();
  await settingsSelect.selectOption("engine");
  await expect(page.getByText("OpenCode Engine")).toBeVisible();

  await page.goto("/#/integrations");
  const integrationsSelect = page.getByRole("combobox", {
    name: "Seções de integrações",
  });
  await expect(integrationsSelect).toBeVisible();
  await integrationsSelect.selectOption("google");
  await expect(page.getByText("OAuth2 loopback + PKCE")).toBeVisible();

  await page.setViewportSize({ width: 1100, height: 800 });
  await page.goto("/#/settings");
  await expect(
    page.getByRole("tablist", { name: "Seções de configuração" }),
  ).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Seções de configuração" }),
  ).toBeHidden();
});
