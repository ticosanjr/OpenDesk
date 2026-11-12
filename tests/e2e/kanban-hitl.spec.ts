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

  await card.getByRole("button", { name: "Responder", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Qual formato você quer para a entrega?");
  await dialog.getByRole("button", { name: "Markdown" }).click();
  await dialog.getByRole("button", { name: "Enviar resposta" }).click();

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
  const seeded = await request.post(`${auth.baseURL}/api/test/seed`, {
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
  expect(seeded.ok()).toBeTruthy();

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
