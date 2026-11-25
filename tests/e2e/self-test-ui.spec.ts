import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("self-test runs from settings and reports every step", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Self Test UI");
  await page.goto("/#/settings?section=diagnostics");
  await expect(page.getByRole("heading", { name: "Testar tudo" })).toBeVisible();
  await page.getByRole("button", { name: "Testar tudo (rápido)" }).click();
  await expect(page.getByText("aprovado")).toBeVisible({ timeout: 15_000 });
  for (const step of ["Banco SQLite", "Motor OpenCode", "Tarefa", "Rotina", "Receita"]) {
    await expect(page.getByText(step, { exact: true })).toBeVisible();
  }
  await expect(page.getByText("Limpeza", { exact: true })).toBeVisible();
  await expect(page.getByText("registros temporários removidos")).toBeVisible();
});
