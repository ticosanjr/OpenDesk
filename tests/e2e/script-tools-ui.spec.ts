import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("script tools can be created and tested from Integrations", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Script Tools UI");

  await page.goto("/#/integrations?section=scripts");
  await expect(page.getByRole("heading", { name: "Ferramentas (Scripts MCP)" })).toBeVisible();
  await page.getByRole("button", { name: "Nova ferramenta" }).first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Slug").fill("ui_echo");
  await dialog.getByLabel("Nome", { exact: true }).fill("Echo UI");
  await dialog.getByLabel("Código do script").fill("cat");
  await dialog.getByRole("button", { name: "Salvar" }).click();
  await expect(page.getByText("script_ui_echo")).toBeVisible();

  await page.getByRole("button", { name: "Editar" }).first().click();
  const edit = page.getByRole("dialog");
  await edit.getByRole("button", { name: "Testar" }).click();
  await expect(edit.locator("pre").first()).toContainText("{}");
  await edit.getByRole("button", { name: "Cancelar" }).click();
});
