import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("kanban separates human work from automation executions", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Kanban Origins");
  const auth = api();
  const create = async (title: string, origin: string) => {
    const response = await request.post(
      `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
      {
        headers: auth.headers,
        data: {
          title,
          description: `Descrição de ${title}`,
          execution_mode: "BUILD_DIRECT",
          origin,
        },
      },
    );
    expect(response.ok()).toBeTruthy();
  };
  await create("Trabalho manual", "MANUAL");
  await create("Receita Alpha", "RECIPE");
  await create("Receita Beta", "RECIPE");
  await create("Entrada do barramento", "IO_BUS");

  await page.goto("/#/kanban");
  await expect(page.getByText("Trabalho manual").first()).toBeVisible();
  await expect(page.getByText("Receita Alpha")).toHaveCount(0);
  await expect(page.getByText("Entrada do barramento")).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Automações/ })).toHaveCount(0);

  const scoped = await request.get(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}&scope=work`,
    { headers: auth.headers },
  );
  expect(scoped.ok()).toBeTruthy();
  const workTasks = (await scoped.json()) as Array<{ title: string }>;
  expect(workTasks.map((task) => task.title)).toEqual(["Trabalho manual"]);

  await page.goto("/#/runs");
  await expect(
    page.getByLabel("Filtrar por origem da execução"),
  ).toBeVisible();
  await expect(
    page.getByLabel("Filtrar por origem da execução").locator("option", {
      hasText: "automações",
    }),
  ).toHaveCount(1);
});

test("deep links open automation tasks outside the work board", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Kanban Deep Link");
  const auth = api();
  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Automação profunda",
        description: "Executada por uma receita.",
        execution_mode: "BUILD_DIRECT",
        origin: "RECIPE",
      },
    },
  );
  expect(created.ok()).toBeTruthy();
  const task = (await created.json()) as { id: string };

  await page.goto(`/#/kanban?task=${task.id}`);
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("Automação profunda").first()).toBeVisible();
});

test("creating an identical task reuses the active one", async ({ request }) => {
  const workspace = await seedWorkspace(request, "Task Dedupe");
  const auth = api();
  const payload = {
    title: "Relatório semanal",
    description: "Consolide os números da semana.",
    execution_mode: "BUILD_DIRECT",
  };
  const first = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    { headers: auth.headers, data: payload },
  );
  expect(first.ok()).toBeTruthy();
  const firstTask = (await first.json()) as {
    id: string;
    deduplicated?: boolean;
    origin: string;
  };
  expect(firstTask.deduplicated).toBe(false);
  expect(firstTask.origin).toBe("MANUAL");

  const second = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    { headers: auth.headers, data: payload },
  );
  const secondTask = (await second.json()) as {
    id: string;
    deduplicated?: boolean;
  };
  expect(secondTask.deduplicated).toBe(true);
  expect(secondTask.id).toBe(firstTask.id);
});

test("queue settings persist the dedupe window", async ({ page, request }) => {
  await seedWorkspace(request, "Queue Settings");
  const auth = api();
  await request.put(`${auth.baseURL}/api/settings/dedupe`, {
    headers: auth.headers,
    data: { enabled: true, cooldown_minutes: 5 },
  });

  await page.goto("/#/settings?section=queue");
  await expect(page.getByText("Fila e duplicatas")).toBeVisible();
  const toggle = page.getByRole("switch", { name: "Evitar tarefas duplicadas" });
  await expect(toggle).toHaveAttribute("aria-checked", "true");
  const cooldown = page.getByLabel("Carência após conclusão em minutos");
  await cooldown.fill("7");
  await page.getByRole("button", { name: "Salvar preferências" }).click();

  await expect
    .poll(async () => {
      const response = await request.get(`${auth.baseURL}/api/settings/dedupe`, {
        headers: auth.headers,
      });
      return (await response.json()) as {
        enabled: boolean;
        cooldown_minutes: number;
      };
    })
    .toEqual({ enabled: true, cooldown_minutes: 7 });

  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-checked", "false");
  await page.getByRole("button", { name: "Salvar preferências" }).click();
  await expect
    .poll(async () => {
      const response = await request.get(`${auth.baseURL}/api/settings/dedupe`, {
        headers: auth.headers,
      });
      return ((await response.json()) as { enabled: boolean }).enabled;
    })
    .toBe(false);

  await request.put(`${auth.baseURL}/api/settings/dedupe`, {
    headers: auth.headers,
    data: { enabled: true, cooldown_minutes: 5 },
  });
});
