import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

async function createTask(
  request: APIRequestContext,
  workspaceId: string,
  data: Record<string, unknown>,
): Promise<string> {
  const auth = api();
  const response = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`,
    { headers: auth.headers, data },
  );
  expect(response.ok()).toBeTruthy();
  return ((await response.json()) as { id: string }).id;
}

async function moveTask(
  request: APIRequestContext,
  taskId: string,
  status: string,
): Promise<void> {
  const auth = api();
  const response = await request.post(
    `${auth.baseURL}/api/tasks/${taskId}/transition`,
    { headers: auth.headers, data: { status } },
  );
  expect(response.ok()).toBeTruthy();
}

async function cardHeights(page: Page, selector: string): Promise<number[]> {
  return page.locator(selector).evaluateAll((elements) =>
    elements.map((element) => element.getBoundingClientRect().height),
  );
}

async function assertConsistent(
  page: Page,
  selector: string,
  minHeight: number,
): Promise<void> {
  const heights = await cardHeights(page, selector);
  expect(heights.length).toBeGreaterThan(2);
  // Cards grow with their content now; the invariant is a readable minimum
  // plus consistent width, not one frozen pixel height for every card.
  expect(Math.min(...heights)).toBeGreaterThanOrEqual(minHeight - 1.5);
  const widths = await page.locator(selector).evaluateAll((elements) =>
    elements.map((element) => element.getBoundingClientRect().width),
  );
  expect(Math.max(...widths) - Math.min(...widths)).toBeLessThanOrEqual(1.5);
}

async function assertNoInternalOverflow(page: Page, selector: string): Promise<void> {
  const overflowing = await page.locator(selector).evaluateAll((elements) =>
    elements
      .filter((element) => element.scrollHeight > element.clientHeight + 2)
      .map((element) => ({
        label: element.getAttribute("aria-label") ?? element.textContent?.slice(0, 50),
        scroll: element.scrollHeight,
        client: element.clientHeight,
      })),
  );
  expect(overflowing).toEqual([]);
}

test("task, routine and attention cards keep a readable minimum and consistent width", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Cards");

  await createTask(request, workspace.id, {
    title: "Curta",
    description: "Curta.",
    tags: [],
  });
  await createTask(request, workspace.id, {
    title:
      "Tarefa com título muito longo que precisa ocupar duas linhas sem empurrar o rodapé do card",
    description:
      "Descrição longa para validar o clamp e a altura fixa do card independentemente do conteúdo. ".repeat(4),
    assigned_agent: "explore",
    preferred_model: "opencode-go/glm-5.3",
    tags: ["um", "dois", "três", "quatro", "cinco", "seis"],
  });
  const blocked = await createTask(request, workspace.id, {
    title: "Bloqueada",
    description: "Sem agente e sem execução.",
    tags: [],
  });
  await moveTask(request, blocked, "BLOCKED");
  const running = await request.post(`${auth.baseURL}/api/test/seed`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      task: {
        title: "Em andamento",
        description: "Rodando.",
        status: "IN_PROGRESS",
      },
    },
  });
  expect(running.ok()).toBeTruthy();

  await page.goto("/#/kanban");
  await expect(page.getByRole("heading", { name: "Kanban & Fila" })).toBeVisible();
  const fakeRunning = page
    .locator('[data-card="task"]')
    .filter({ hasText: "Em andamento" });
  await expect(fakeRunning).toHaveAttribute("data-workflow-state", "BLOCKED");
  await expect(fakeRunning).toHaveAttribute(
    "data-workflow-reason",
    "in_progress_without_run",
  );

  const recipe = await request.put(
    `${auth.baseURL}/api/recipes/receita-cards?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        yaml: `id: receita-cards
name: Receita Cards
enabled: true
triggers: []
flow:
  - type: step
    id: passo-1
    name: Passo
    mode: build
    prompt: ok
`,
      },
    },
  );
  expect(recipe.ok()).toBeTruthy();

  for (const schedule of [
    {
      name: "Rotina curta",
      description: "Curta.",
      cron_expression: "0 8 * * *",
      timezone: "America/Sao_Paulo",
      command_or_prompt: "Resumo",
      trigger_type: "CRON",
      notify_signal: true,
      notify_only_on_change: true,
      agent_name: "build",
      is_active: true,
    },
    {
      name: "Rotina com nome bem longo para validar o clamp do cabeçalho e dos badges",
      description: "Rotina com vários metadados para testar a linha fixa.",
      cron_expression: "0 9 * * 1",
      timezone: "Europe/Rome",
      command_or_prompt:
        "Prompt longo para validar duas linhas sem alterar a altura do card. ".repeat(3),
      trigger_type: "CRON",
      agent_name: "pesquisador",
      target_kind: "RECIPE",
      recipe_id: "receita-cards",
      router_policy: "BEST_QUALITY",
      is_active: true,
    },
    {
      name: "Rotina pausada",
      description: "Inativa.",
      cron_expression: "0 10 * * *",
      timezone: "UTC",
      command_or_prompt: "Outro prompt",
      trigger_type: "CRON",
      is_active: false,
    },
  ]) {
    const response = await request.post(`${auth.baseURL}/api/schedules`, {
      headers: auth.headers,
      data: { workspace_id: workspace.id, ...schedule },
    });
    expect(response.ok()).toBeTruthy();
  }

  for (const title of [
    "Proposta com título longo para validar a linha de atenção",
    "Segunda proposta",
  ]) {
    const proposal = await request.post(
      `${auth.baseURL}/api/proposals?workspace_id=${workspace.id}`,
      {
        headers: auth.headers,
        data: {
          kind: "AGENT",
          action: "CREATE",
          title,
          description: "Detalhe da proposta.",
          payload: {},
        },
      },
    );
    expect(proposal.ok()).toBeTruthy();
  }
  const attentionTask = await createTask(request, workspace.id, {
    title: "Tarefa bloqueada na atenção",
    description: "Precisa de decisão.",
    tags: [],
  });
  await moveTask(request, attentionTask, "BLOCKED");

  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/#/kanban");
  await expect(page.locator('[data-card="task"]')).toHaveCount(5);
  await assertConsistent(page, '[data-card="task"]', 176);
  await assertNoInternalOverflow(page, '[data-card="task"]');
  await page.screenshot({
    path: test.info().outputPath("kanban-cards-1440.png"),
    fullPage: true,
  });

  await page.goto("/#/routines");
  await expect(page.locator('[data-card="routine"]')).toHaveCount(3);
  await assertConsistent(page, '[data-card="routine"]', 188);
  await assertNoInternalOverflow(page, '[data-card="routine"]');
  await page.getByRole("button", { name: /Mais ações da rotina/ }).first().click();
  await expect(page.getByRole("button", { name: "Histórico" })).toBeVisible();
  await page.screenshot({
    path: test.info().outputPath("routine-cards-1440.png"),
    fullPage: true,
  });

  await page.goto("/#/");
  await expect(page.locator("[data-attention-row]")).toHaveCount(4);
  await assertConsistent(page, "[data-attention-row]", 72);
  await assertNoInternalOverflow(page, "[data-attention-row]");
  await page.screenshot({
    path: test.info().outputPath("attention-cards-1440.png"),
    fullPage: true,
  });

  await page.setViewportSize({ width: 1100, height: 800 });
  await page.goto("/#/kanban");
  await expect(page.locator('[data-card="task"]')).toHaveCount(5);
  await assertConsistent(page, '[data-card="task"]', 176);
  await page.goto("/#/routines");
  await expect(page.locator('[data-card="routine"]')).toHaveCount(3);
  await assertConsistent(page, '[data-card="routine"]', 188);
  await page.goto("/#/");
  await expect(page.locator("[data-attention-row]")).toHaveCount(4);
  await assertConsistent(page, "[data-attention-row]", 72);
});

test("kanban groups by workflow and honors explicit hold", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Workflow Hold");
  const taskId = await createTask(request, workspace.id, {
    title: "Hold E2E",
    description: "Tarefa que deve ficar em espera.",
    tags: [],
  });
  const held = await request.post(
    `${auth.baseURL}/api/tasks/${taskId}/hold`,
    { headers: auth.headers, data: { held: true } },
  );
  expect(held.ok()).toBeTruthy();

  await page.goto("/#/kanban");
  const card = page
    .locator('[data-card="task"]')
    .filter({ hasText: "Hold E2E" });
  await expect(card).toHaveAttribute("data-workflow-state", "HELD");
  await expect(card.getByRole("button", { name: "Retomar" })).toBeVisible();
  await expect(page.locator('[data-column="QUEUE"]')).toContainText("Hold E2E");
  await expect(page.getByText("Atenção", { exact: true })).toBeVisible();
  await expect(page.getByText("Em execução", { exact: true })).toBeVisible();

  await card.getByRole("button", { name: "Retomar" }).click();
  await expect
    .poll(async () => {
      const tasks = await request.get(`${auth.baseURL}/api/tasks`, {
        headers: auth.headers,
      });
      const list = (await tasks.json()) as Array<{
        id: string;
        held: boolean;
        workflow_state: string;
      }>;
      const current = list.find((item) => item.id === taskId);
      return `${current?.held}|${current?.workflow_state}`;
    })
    .toBe("false|READY");
});
