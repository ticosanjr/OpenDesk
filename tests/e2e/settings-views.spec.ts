import { expect, test } from "@playwright/test";
import { seedCompany } from "./fixtures/company";

test("settings expose general, tools and diagnostics views", async ({ page, request }) => {
  await seedCompany(request, "Settings Views");

  await page.goto("/#/settings?section=general");
  await expect(page.getByText("Pasta padrão de workspaces")).toBeVisible();

  await page.goto("/#/settings?section=tools");
  await expect(page.getByText("Ferramentas externas")).toBeVisible();

  await page.goto("/#/settings?section=about");
  await expect(page.getByText("Sobre & diagnóstico")).toBeVisible();
  await page.getByRole("button", { name: "avançado", exact: true }).click();
  await expect(page.getByText("Aplicação").first()).toBeVisible();
  await expect(page.getByText("Banco e caminhos").first()).toBeVisible();
});
