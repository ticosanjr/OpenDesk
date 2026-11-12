import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

const RECIPE_YAML = `id: receita-fase1
name: Receita Fase 1
description: Teste de toggle
enabled: true
triggers: []
flow:
  - type: step
    id: passo-1
    name: Passo único
    mode: build
    agent: build
    effort: low
    prompt: "Resuma o contexto em {{run.output_dir}}/resumo.md"
`;

test("phase 1 switches, explicit model and advanced options", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Phase One");

  const schedule = await request.post(`${auth.baseURL}/api/schedules`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      name: "Rotina Fase 1",
      command_or_prompt: "Executar verificação",
      cron_expression: "0 9 * * 1",
      timezone: "UTC",
    },
  });
  expect(schedule.ok()).toBeTruthy();
  const scheduleId = ((await schedule.json()) as { id: string }).id;

  const recipe = await request.put(
    `${auth.baseURL}/api/recipes/receita-fase1?workspace_id=${workspace.id}`,
    { headers: auth.headers, data: { yaml: RECIPE_YAML } },
  );
  expect(recipe.ok()).toBeTruthy();

  // New routines start paused. The switch is the explicit activation path.
  await page.goto("/#/routines");
  await expect(
    page.getByRole("switch", { name: "Ativar rotina Rotina Fase 1" }),
  ).toBeVisible();
  await page
    .getByRole("switch", { name: "Ativar rotina Rotina Fase 1" })
    .click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Ativar rotina", exact: true })
    .click();
  await expect(
    page.getByRole("switch", { name: "Desativar rotina Rotina Fase 1" }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("switch", { name: "Desativar rotina Rotina Fase 1" }),
  ).toBeVisible();
  const routines = (await (
    await request.get(`${auth.baseURL}/api/schedules`, { headers: auth.headers })
  ).json()) as Array<{ id: string; is_active: boolean }>;
  expect(routines.find((item) => item.id === scheduleId)?.is_active).toBe(true);
  await page
    .getByRole("switch", { name: "Desativar rotina Rotina Fase 1" })
    .click();
  await expect(
    page.getByRole("switch", { name: "Ativar rotina Rotina Fase 1" }),
  ).toBeVisible();

  // New recipes also start disabled; toggling is explicit.
  await page.goto("/#/recipes");
  await expect(
    page.getByRole("switch", { name: "Ativar receita Receita Fase 1" }),
  ).toBeVisible();
  await page
    .getByRole("switch", { name: "Ativar receita Receita Fase 1" })
    .click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Ativar receita", exact: true })
    .click();
  await expect(
    page.getByRole("switch", { name: "Desativar receita Receita Fase 1" }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("switch", { name: "Desativar receita Receita Fase 1" }),
  ).toBeVisible();
  const recipes = (await (
    await request.get(`${auth.baseURL}/api/recipes?workspace_id=${workspace.id}`, {
      headers: auth.headers,
    })
  ).json()) as { recipes: Array<{ id: string; is_enabled: boolean }> };
  expect(
    recipes.recipes.find((item) => item.id === "receita-fase1")?.is_enabled,
  ).toBe(true);
  await page
    .getByRole("switch", { name: "Desativar receita Receita Fase 1" })
    .click();
  await expect(
    page.getByRole("switch", { name: "Ativar receita Receita Fase 1" }),
  ).toBeVisible();

  // Task modal keeps advanced fields collapsed by default.
  await page.goto("/#/kanban");
  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  const more = dialog.getByRole("button", { name: "Mais opções" });
  await expect(more).toHaveAttribute("aria-expanded", "false");
  await expect(dialog.getByText("Modelo", { exact: true })).toHaveCount(0);

  await more.click();
  await expect(more).toHaveAttribute("aria-expanded", "true");
  const routerTab = dialog.getByRole("button", { name: "Roteador automático" });
  await expect(routerTab).toHaveAttribute("aria-pressed", "true");

  await dialog.getByRole("button", { name: "Modelo específico" }).click();
  const modelInput = dialog.getByRole("combobox", { name: "Modelo" });
  await expect(modelInput).toBeVisible();
  await modelInput.fill("opencode-go/glm-5.3");

  await dialog
    .getByPlaceholder("Se vazio, usamos o início da descrição")
    .fill("Tarefa com modelo fixo");
  await dialog
    .getByPlaceholder("Descreva o objetivo, as entradas e os critérios de aceite…")
    .fill("Validar que o modelo escolhido não é trocado pelo roteador.");
  await dialog.getByRole("button", { name: "Criar tarefa" }).click();

  await expect(page.getByText("Tarefa com modelo fixo").first()).toBeVisible();
  const tasks = (await (
    await request.get(`${auth.baseURL}/api/tasks`, { headers: auth.headers })
  ).json()) as Array<{
    id: string;
    title: string;
    preferred_model?: string | null;
    model_override: boolean;
  }>;
  const task = tasks.find((item) => item.title === "Tarefa com modelo fixo");
  expect(task).toBeTruthy();
  expect(task?.preferred_model).toBe("opencode-go/glm-5.3");
  expect(task?.model_override).toBe(true);
});
