import { expect, test } from "@playwright/test";
import { seedCompany } from "./fixtures/company";
import { api } from "./helpers";

test("studio creates a command and persists it to the workspace", async ({
  page,
  request,
}) => {
  const company = await seedCompany(request, "Studio Command");
  await page.goto("/#/studio");
  await page
    .getByRole("main")
    .getByRole("button", { name: /^Workspace ·/ })
    .click();
  await page.getByRole("button", { name: "Comandos", exact: true }).click();
  await page.getByRole("button", { name: "Novo", exact: true }).click();
  await page.getByLabel("ID / slug").fill("fechamento-mensal");
  await page.getByLabel("Descrição").fill("Resume o fechamento do mês");
  await page
    .getByLabel("Template")
    .fill("Resuma o fechamento do mês com números e pendências.");
  await page.getByRole("button", { name: "Salvar", exact: true }).click();
  await expect(
    page.getByText("fechamento-mensal").first(),
  ).toBeVisible();

  const auth = api();
  const response = await request.get(
    `${auth.baseURL}/api/studio/commands?scope=workspace&workspace_id=${company.workspace.id}`,
    { headers: auth.headers },
  );
  expect(response.ok()).toBeTruthy();
  const commands = (await response.json()) as Array<{ id: string }>;
  expect(commands.some((command) => command.id === "fechamento-mensal")).toBe(
    true,
  );
});
