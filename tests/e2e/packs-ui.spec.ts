import { expect, test } from "@playwright/test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { api, installPack, seedWorkspace } from "./helpers";

test("starter pack conflict lets the user keep or replace each file", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Packs Conflict");
  const agent = `${workspace.root}/.opencode/agents/assistente.md`;
  mkdirSync(`${workspace.root}/.opencode/agents`, { recursive: true });
  writeFileSync(agent, "custom do usuário");

  await page.goto("/#/packs");
  await page.getByRole("button", { name: /Essenciais/ }).first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("conflitos", { exact: false })).toBeVisible();
  await dialog.getByRole("button", { name: /Adicionar ao workspace/ }).click();
  await expect(page.getByText("Essenciais adicionado").first()).toBeVisible();
  expect(readFileSync(agent, "utf8")).toBe("custom do usuário");
  expect(existsSync(`${workspace.root}/.opencode/agents/pesquisador.md`)).toBe(true);

  await page.getByRole("button", { name: /Essenciais/ }).first().click();
  const replaceDialog = page.getByRole("dialog");
  const conflict = replaceDialog.getByRole("checkbox", {
    name: /assistente\.md/,
  });
  await conflict.check();
  await expect(conflict).toBeChecked();
  await replaceDialog.getByRole("button", { name: /Reinstalar/ }).click();
  await expect
    .poll(() => readFileSync(agent, "utf8"))
    .toContain("Assistente");

});

test("starter pack repair recreates missing files and uninstall preserves edits", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Packs Repair");
  await installPack(request, workspace.id, "essenciais");

  const assistant = `${workspace.root}/.opencode/agents/assistente.md`;
  const researcher = `${workspace.root}/.opencode/agents/pesquisador.md`;
  rmSync(assistant);
  await page.goto("/#/packs");
  await expect(page.getByText("parcial", { exact: true }).first()).toBeVisible();
  await page.getByRole("button", { name: /Essenciais/ }).first().click();
  await expect(page.getByText(/Faltam 1 arquivo/).first()).toBeVisible();
  await page.getByRole("dialog").getByRole("button", { name: /Reparar/ }).click();
  await expect(page.getByText("Essenciais adicionado").first()).toBeVisible();
  expect(existsSync(assistant)).toBe(true);

  writeFileSync(researcher, "pesquisa personalizada");
  await page.getByRole("button", { name: /Essenciais/ }).first().click();
  await page.getByRole("dialog").getByRole("button", { name: /Remover/ }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Remover pacote" }).click();
  await expect.poll(() => existsSync(researcher)).toBe(true);
  expect(readFileSync(researcher, "utf8")).toBe("pesquisa personalizada");

});

test("external capability catalog filters and opens a recommended workspace", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Packs Resources");
  await page.goto("/#/packs");
  await expect(page.getByText("Descobrir capacidades")).toBeVisible();

  await page.getByRole("button", { name: "MCP", exact: true }).click();
  const smithery = page.locator("article").filter({ hasText: "Smithery" });
  await expect(smithery).toBeVisible();
  await smithery.getByRole("button", { name: "Usar template recomendado" }).click();
  await expect(
    page.getByRole("dialog").getByText(/Criar workspace:/),
  ).toBeVisible();
});

test("package tabs, global search and individual item install work", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Packs Items");
  await page.goto("/#/packs");
  await page.getByPlaceholder(/Buscar nos workspaces/).fill("Dropshipping");
  await expect(page.getByText("Dropshipping Radar").first()).toBeVisible();
  await page.getByPlaceholder(/Buscar nos workspaces/).fill("");
  await page.getByRole("button", { name: /Pacotes \(/ }).click();
  await expect(
    page.getByRole("heading", { name: "Adicionar capacidades" }),
  ).toBeVisible();
  await page.getByRole("button", { name: /^Essenciais/ }).first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("button", { name: /Agentes/ })).toBeVisible();
  await dialog.getByRole("button", { name: "Instalar" }).first().click();
  await expect(page.getByText(/agent assistente instalado/).first()).toBeVisible();
  await expect
    .poll(() => existsSync(`${workspace.root}/.opencode/agents/assistente.md`))
    .toBe(true);
});

test("workspace template creates a ready-to-use workspace from the UI", async ({
  page,
  request,
}) => {
  const auth = api();
  await seedWorkspace(request, "Packs Template");
  const root = `/tmp/opendesk-e2e/template-ui-${Date.now()}`;
  const name = `Conteúdo UI ${Date.now()}`;

  await page.goto("/#/packs");
  await page
    .locator("button")
    .filter({ hasText: "Conteúdo" })
    .last()
    .click();
  const dialog = page.getByRole("dialog");
  const textboxes = dialog.getByRole("textbox");
  await textboxes.first().fill(name);
  await textboxes.nth(1).fill(root);
  await dialog.getByRole("button", { name: /Criar workspace/ }).click();
  await expect(page.getByText("Workspace criado").first()).toBeVisible({
    timeout: 60_000,
  });
  let createdId: string | undefined;
  await expect
    .poll(
      async () => {
        const response = await request.get(`${auth.baseURL}/api/workspaces`, {
          headers: auth.headers,
        });
        const list = (await response.json()) as Array<{ id: string; name: string }>;
        createdId = list.find((item) => item.name === name)?.id;
        return Boolean(createdId);
      },
      { timeout: 60_000 },
    )
    .toBe(true);
  expect(existsSync(`${root}/.opencode/agents/roteirista-youtube.md`)).toBe(true);
  expect(existsSync(`${root}/README.md`)).toBe(true);

  if (createdId) {
    await request.delete(`${auth.baseURL}/api/workspaces/${createdId}`, {
      headers: auth.headers,
    });
  }
});
