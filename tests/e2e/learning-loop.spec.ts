import { expect, test } from "@playwright/test";
import { existsSync, readFileSync } from "node:fs";
import { seedCompany, seedOperational } from "./fixtures/company";

test("learning scans a failure, promotes a skill and remembers the lesson", async ({
  page,
  request,
}) => {
  const company = await seedCompany(request, "Learning Loop");
  await seedOperational(request, company, {
    failedRun: true,
    artifacts: false,
    hitl: false,
    proposal: false,
    permission: false,
  });

  await page.goto("/#/learning");
  await page.getByRole("button", { name: "Analisar execuções" }).click();
  await expect(
    page.getByText("Falha recorrente: timeout").first(),
  ).toBeVisible();

  await page.getByRole("button", { name: /Criar proposta/ }).first().click();
  await expect(
    page.getByText("Propostas aguardando aprovação").first(),
  ).toBeVisible();
  await page.getByRole("button", { name: "Aprovar", exact: true }).first().click();
  await expect(
    page.getByText("Propostas aguardando aprovação").first(),
  ).toBeHidden({ timeout: 15_000 });

  await page.getByRole("button", { name: "Promovidas", exact: true }).click();
  await page.getByRole("button", { name: "Salvar na memória", exact: true }).first().click();
  await expect(
    page.getByText("Lição adicionada à MEMORY.md").first(),
  ).toBeVisible();

  const memory = `${company.workspace.root}/.opencode/MEMORY.md`;
  expect(existsSync(memory)).toBe(true);
  expect(readFileSync(memory, "utf8")).toContain("Aprendizado:");
});
