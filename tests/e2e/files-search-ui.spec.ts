import { expect, test } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { seedWorkspace } from "./helpers";

test("files page searches the whole workspace and opens a nested result", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Files Search");
  mkdirSync(`${workspace.root}/output/2026-11/relatorios`, { recursive: true });
  writeFileSync(
    `${workspace.root}/output/2026-11/relatorios/fechamento-final.md`,
    "# Fechamento\n\nconteudo unico E2E",
  );

  await page.goto("/#/files");
  await page.waitForTimeout(300);
  await expect(page.getByRole("heading", { name: "Arquivos" })).toBeVisible();
  await page.getByRole("button", { name: "Todo o workspace" }).click();
  await page
    .getByPlaceholder("Procurar em todo o workspace")
    .fill("fechamento");
  await expect(page.getByText("fechamento-final.md").first()).toBeVisible();
  await page.getByText("fechamento-final.md").first().click();
  await expect(page.getByText(/conteudo unico E2E/).first()).toBeVisible();
});

test("files explorer lists workspace folders and previews a nested file", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Files Explorer");
  mkdirSync(`${workspace.root}/output/2026-11/relatorios`, { recursive: true });
  writeFileSync(
    `${workspace.root}/output/2026-11/relatorios/resumo.md`,
    "# Resumo\n\npreview do explorador E2E",
  );

  await page.goto("/#/files");
  await expect(page.getByRole("heading", { name: "Arquivos" })).toBeVisible();
  await page.getByRole("button", { name: /^output/ }).first().click();
  await page.getByRole("button", { name: /^2026-11/ }).first().click();
  await page.getByRole("button", { name: /^relatorios/ }).first().click();
  await page.getByRole("button", { name: /^resumo\.md/ }).first().click();
  await expect(page.getByText(/preview do explorador E2E/).first()).toBeVisible();
});
