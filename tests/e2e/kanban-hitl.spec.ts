import { expect, test, type APIRequestContext } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

async function createTask(
  request: APIRequestContext,
  workspaceId: string,
  title: string,
): Promise<string> {
  const auth = api();
  const response = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        title,
        description: "Tarefa para validar o HITL direto no Kanban.",
        tags: [],
      },
    },
  );
  expect(response.ok()).toBeTruthy();
  return ((await response.json()) as { id: string }).id;
}

test("attention card shows the agent question and resolves it inline", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Kanban HITL");
  const taskId = await createTask(request, workspace.id, "Responder no Kanban");
  const auth = api();
  const asked = await request.post(
    `${auth.baseURL}/api/tasks/${taskId}/interactions`,
    {
      headers: auth.headers,
      data: {
        question: "Qual formato você quer para a entrega?",
        options: ["PDF", "Markdown"],
        allow_custom: true,
        timeout_minutes: 5,
      },
    },
  );
  expect(asked.ok()).toBeTruthy();

  await page.goto("/#/kanban");
  await expect(page.locator('[data-column="ATTENTION"]')).toContainText(
    "Responder no Kanban",
  );
  const card = page.locator('[data-card="task"]').filter({ hasText: "Responder no Kanban" });
  await expect(card).toContainText("O agente perguntou");
  await expect(card).toContainText("Qual formato você quer para a entrega?");
  await expect(card.getByRole("button", { name: "Responder", exact: true })).toBeVisible();

  const dialog = page.getByRole("dialog", { name: "Decisão necessária" });
  await expect(dialog).toContainText("Qual formato você quer para a entrega?");
  await dialog.getByRole("button", { name: "Markdown" }).click();
  await dialog.getByRole("button", { name: "Enviar escolha" }).click();

  await expect(dialog).not.toBeVisible();
  await expect(page.getByText("O agente perguntou")).toHaveCount(0);
  await expect
    .poll(async () => {
      const response = await request.get(`${auth.baseURL}/api/tasks/${taskId}`, {
        headers: auth.headers,
      });
      const body = (await response.json()) as { phase_status?: string | null };
      return body.phase_status;
    })
    .not.toBe("awaiting_input");
});

test("permission requests are answerable from the Kanban card", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Kanban Permission");
  const auth = api();
  let seeded = await request.post(`${auth.baseURL}/api/test/seed`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      task: {
        title: "Permissão no Kanban",
        description: "O agente precisa de um comando.",
        status: "IN_REVIEW",
      },
      permission: {
        action: "bash",
        resources: ["npm install"],
        session_id: "kanban-hitl",
      },
    },
  });
  // Another worker may still be releasing SQLite during teardown; retry once.
  for (let attempt = 0; !seeded.ok() && attempt < 3; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 250 * (attempt + 1)));
    seeded = await request.post(`${auth.baseURL}/api/test/seed`, {
      headers: auth.headers,
      data: {
        workspace_id: workspace.id,
        task: {
          title: "Permissão no Kanban",
          description: "O agente precisa de um comando.",
          status: "IN_REVIEW",
        },
        permission: {
          action: "bash",
          resources: ["npm install"],
          session_id: "kanban-hitl",
        },
      },
    });
  }
  expect(seeded.ok(), await seeded.text()).toBeTruthy();

  await page.goto("/#/kanban");
  await expect(page.locator('[data-column="ATTENTION"]')).toContainText(
    "Permissão no Kanban",
  );
  const card = page
    .locator('[data-card="task"]')
    .filter({ hasText: "Permissão no Kanban" });
  await expect(card).toContainText("Permissão para bash");
  await expect(card).toContainText("npm install");

  await card.getByRole("button", { name: "Responder", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("O agente pediu permissão");
  await dialog.getByRole("button", { name: "Permitir uma vez" }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.getByText("Permissão para bash")).toHaveCount(0);
});

test("approval and multiple interactions are visible on attention cards", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Kanban Approval");
  const auth = api();
  const seeded = await request.post(`${auth.baseURL}/api/test/seed`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      task: {
        title: "Aprovar plano no card",
        description: "Plano pronto para revisão.",
        status: "IN_REVIEW",
        phase_status: "awaiting_approval",
      },
    },
  });
  expect(seeded.ok()).toBeTruthy();

  const task = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Múltiplas perguntas",
        description: "Duas decisões pendentes.",
        tags: [],
      },
    },
  );
  const taskId = ((await task.json()) as { id: string }).id;
  for (const question of ["Primeira pergunta?", "Segunda pergunta?"]) {
    const asked = await request.post(
      `${auth.baseURL}/api/tasks/${taskId}/interactions`,
      {
        headers: auth.headers,
        data: { question, options: ["Sim", "Não"], allow_custom: false, timeout_minutes: 5 },
      },
    );
    expect(asked.ok()).toBeTruthy();
  }

  await page.goto("/#/kanban");
  const approval = page
    .locator('[data-card="task"]')
    .filter({ hasText: "Aprovar plano no card" });
  await expect(approval).toContainText("Plano aguardando aprovação");
  await expect(
    approval.getByRole("button", { name: "Aprovar plano", exact: true }),
  ).toBeVisible();

  const multiple = page
    .locator('[data-card="task"]')
    .filter({ hasText: "Múltiplas perguntas" });
  await expect(multiple).toBeVisible();
  const dialog = page.getByRole("dialog", { name: "Decisão necessária" });
  await expect(dialog).toContainText("Primeira pergunta?");
  await dialog.getByRole("button", { name: "Sim" }).click();
  await dialog.getByRole("button", { name: "Enviar escolha" }).click();
  await expect(dialog).toContainText("Segunda pergunta?");
});

test("permission can be denied from the Kanban modal", async ({ page, request }) => {
  const workspace = await seedWorkspace(request, "Kanban Deny");
  const auth = api();
  let seeded = await request.post(`${auth.baseURL}/api/test/seed`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      task: {
        title: "Negar permissão no Kanban",
        description: "O agente pediu acesso.",
        status: "IN_REVIEW",
      },
      permission: {
        action: "bash",
        resources: ["rm -rf build"],
        session_id: "kanban-deny",
      },
    },
  });
  for (let attempt = 0; !seeded.ok() && attempt < 3; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 250 * (attempt + 1)));
    seeded = await request.post(`${auth.baseURL}/api/test/seed`, {
      headers: auth.headers,
      data: {
        workspace_id: workspace.id,
        task: {
          title: "Negar permissão no Kanban",
          description: "O agente pediu acesso.",
          status: "IN_REVIEW",
        },
        permission: {
          action: "bash",
          resources: ["rm -rf build"],
          session_id: "kanban-deny",
        },
      },
    });
  }
  expect(seeded.ok(), await seeded.text()).toBeTruthy();

  await page.goto("/#/kanban");
  const card = page
    .locator('[data-card="task"]')
    .filter({ hasText: "Negar permissão no Kanban" });
  await card.getByRole("button", { name: "Responder", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Negar", exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.getByText("Permissão para bash")).toHaveCount(0);
});
