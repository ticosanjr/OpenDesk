import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

async function saveRecipe(
  request: Parameters<typeof seedWorkspace>[0],
  workspaceId: string,
  id: string,
  yaml: string,
) {
  const auth = api();
  const response = await request.put(
    `${auth.baseURL}/api/recipes/${id}?workspace_id=${workspaceId}`,
    { headers: auth.headers, data: { yaml } },
  );
  expect(response.ok()).toBeTruthy();
}

test("recipe visual flow can be reordered and accept multiple triggers", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Recipes Flow");
  const id = "receita-ui-flow";
  await saveRecipe(
    request,
    workspace.id,
    id,
    `id: ${id}
name: Receita UI Flow
enabled: true
triggers: []
flow:
  - type: step
    id: passo-1
    name: Passo 1
    mode: build
    prompt: primeiro
  - type: step
    id: passo-2
    name: Passo 2
    mode: build
    prompt: segundo
`,
  );

  await page.goto("/#/recipes");
  await expect(page.getByText("Receita UI Flow").first()).toBeVisible();
  await expect(page.getByText("Ordem de execução")).toBeVisible();
  await page.locator('button[title="Mover para baixo"]').first().click();
  const editor = page.getByLabel("Editor YAML da receita");
  await expect
    .poll(async () => {
      const value = await editor.inputValue();
      return value.indexOf("id: passo-2") < value.indexOf("id: passo-1");
    })
    .toBe(true);
  await page.getByRole("button", { name: /Adicionar gatilho/ }).click();
  await expect(page.getByLabel("Tipo do gatilho 1")).toBeVisible();
  await page.getByRole("button", { name: /Adicionar gatilho/ }).click();
  await expect(page.getByLabel("Tipo do gatilho 2")).toBeVisible();
  await page.getByLabel("Tipo do gatilho 1").selectOption("WEBHOOK");
  await page.getByLabel("Tipo do gatilho 2").selectOption("CRON");
  await page.getByRole("button", { name: /Salvar/ }).first().click();
  await expect
    .poll(async () => {
      const detail = await request.get(`${auth.baseURL}/api/recipes/${id}`, {
        headers: auth.headers,
      });
      const yaml = ((await detail.json()) as { yaml: string }).yaml;
      return {
        reordered: yaml.indexOf("id: passo-2") < yaml.indexOf("id: passo-1"),
        webhook: yaml.includes("type: WEBHOOK"),
        cron: yaml.includes("type: CRON"),
      };
    })
    .toEqual({ reordered: true, webhook: true, cron: true });

  const detail = await request.get(`${auth.baseURL}/api/recipes/${id}`, {
    headers: auth.headers,
  });
  const yaml = ((await detail.json()) as { yaml: string }).yaml;
  expect(yaml.indexOf("id: passo-2")).toBeLessThan(yaml.indexOf("id: passo-1"));
  expect(yaml).toContain("type: WEBHOOK");
  expect(yaml).toContain("type: CRON");

});

test("recipe run modal sends the typed context", async ({ page, request }) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Recipes Context");
  const id = "receita-ui-context";
  await saveRecipe(
    request,
    workspace.id,
    id,
    `id: ${id}
name: Receita Contexto
enabled: true
triggers: []
flow:
  - type: step
    id: passo-1
    name: Passo 1
    mode: build
    prompt: "Use {{trigger.context}}"
`,
  );

  // New recipes start disabled; enable explicitly for this run-path test.
  const enabled = await request.post(
    `${auth.baseURL}/api/recipes/${id}/enabled?workspace_id=${workspace.id}`,
    { headers: auth.headers, data: { enabled: true } },
  );
  expect(enabled.ok()).toBeTruthy();

  await page.goto("/#/recipes");
  await expect(page.getByText("Receita Contexto").first()).toBeVisible();
  await page.getByRole("button", { name: /Dry-run/ }).click();
  const dialog = page.getByRole("dialog");
  await dialog
    .getByPlaceholder(/analise o contrato/)
    .fill("contexto E2E de teste");
  await dialog.getByRole("button", { name: /Iniciar dry-run/ }).click();
  await expect(page.getByText("Dry-run iniciado").first()).toBeVisible();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/recipes/runs?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as {
        runs: Array<{ recipe_id: string; trigger_context_json: string }>;
      };
      return body.runs.some(
        (run) =>
          run.recipe_id === id &&
          run.trigger_context_json.includes("contexto E2E de teste"),
      );
    })
    .toBe(true);

});
