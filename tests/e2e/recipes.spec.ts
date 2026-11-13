import { expect, test } from "@playwright/test";
import { api } from "./helpers";


test("recipes CRUD, dry-run and tools inventory", async ({ page, request }) => {
  const auth = api();
  const workspaces = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  let workspaceId = workspaces[0]?.id;
  if (!workspaceId) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: { name: "Recipes", root_path: "/tmp/opendesk-e2e/recipes" },
    });
    workspaceId = ((await created.json()) as { id: string }).id;
  }

  const yaml = `id: receita-e2e
name: Receita E2E
enabled: true
trigger:
  type: MANUAL
  config: {}
steps:
  - id: passo-1
    name: Passo 1
    mode: build
    agent: build
    prompt: "Execute {{run.output_dir}}"
hitl_checkpoints: []
sinks: []
`;
  const saved = await request.put(
    `${auth.baseURL}/api/recipes/receita-e2e?workspace_id=${workspaceId}`,
    { headers: auth.headers, data: { yaml } },
  );
  expect(saved.ok()).toBeTruthy();
  const index = (await saved.json()) as { id: string; name: string };
  expect(index.name).toBe("Receita E2E");
  const enabled = await request.post(
    `${auth.baseURL}/api/recipes/receita-e2e/enabled?workspace_id=${workspaceId}`,
    { headers: auth.headers, data: { enabled: true } },
  );
  expect(enabled.ok()).toBeTruthy();

  const list = await request.get(
    `${auth.baseURL}/api/recipes?workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  expect(((await list.json()) as { recipes: unknown[] }).recipes.length).toBeGreaterThan(0);

  await page.goto("/#/recipes");
  await expect(page.getByText("Receitas", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("Receita E2E").first()).toBeVisible();
  await page.getByText("Receita E2E").first().click();
  await expect(page.getByRole("heading", { name: "Fluxo visual" })).toBeVisible();
  await expect(page.getByText("YAML", { exact: true })).toBeVisible();

  const detail = await request.get(`${auth.baseURL}/api/recipes/receita-e2e`, {
    headers: auth.headers,
  });
  expect(((await detail.json()) as { yaml: string }).yaml).toContain("passo-1");

  const run = await request.post(`${auth.baseURL}/api/recipes/receita-e2e/run`, {
    headers: auth.headers,
    data: { dry_run: true, trigger_context: { manual: true } },
  });
  expect(run.ok()).toBeTruthy();
  const recipeRun = (await run.json()) as { id: string; status: string };
  expect(recipeRun.status).toBe("RUNNING_STEP");

  await expect
    .poll(
      async () => {
        const response = await request.get(
          `${auth.baseURL}/api/recipes/runs/${recipeRun.id}`,
          { headers: auth.headers },
        );
        const body = (await response.json()) as { run: { status: string } };
        return body.run.status;
      },
      { timeout: 10000 },
    )
    .toBe("COMPLETED");

  const runs = await request.get(
    `${auth.baseURL}/api/recipes/runs?workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  expect(((await runs.json()) as { runs: unknown[] }).runs.length).toBeGreaterThan(0);

  const tools = await request.get(`${auth.baseURL}/api/io/tools`, {
    headers: auth.headers,
  });
  const toolList = (await tools.json()) as Array<{ name: string }>;
  expect(toolList.some((tool) => tool.name === "rsync")).toBeTruthy();
});
