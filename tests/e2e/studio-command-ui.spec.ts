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
  // The editor header already shows the typed ID before the save. Wait for the
  // saved state (ID becomes read-only) instead of racing the PUT.
  await expect(page.getByLabel("ID / slug")).toBeDisabled();

  const auth = api();
  await expect
    .poll(
      async () => {
        const response = await request.get(
          `${auth.baseURL}/api/studio/commands?scope=workspace&workspace_id=${company.workspace.id}`,
          { headers: auth.headers },
        );
        if (!response.ok()) return false;
        const commands = (await response.json()) as Array<{ id: string }>;
        return commands.some((command) => command.id === "fechamento-mensal");
      },
      { timeout: 10_000 },
    )
    .toBe(true);
});
