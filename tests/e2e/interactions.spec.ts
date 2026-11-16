import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("agent question blocks the task and resumes after an answer", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Interactions");
  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Pergunta E2E",
        description: "Tarefa que precisa de esclarecimento.",
        tags: [],
      },
    },
  );
  expect(created.ok()).toBeTruthy();
  const task = (await created.json()) as { id: string };

  const asked = await request.post(
    `${auth.baseURL}/api/tasks/${task.id}/interactions`,
    {
      headers: auth.headers,
      data: {
        question: "Qual formato você quer?",
        options: ["Markdown", "PDF"],
        allow_custom: true,
      },
    },
  );
  expect(asked.ok()).toBeTruthy();

  await page.goto(`/#/kanban?task=${task.id}`);
  const dialog = page.getByRole("dialog", { name: "Decisão necessária" });
  await expect(dialog.getByText("O agente precisa da sua resposta")).toBeVisible();
  await expect(dialog.getByText("Qual formato você quer?")).toBeVisible();
  await dialog.getByRole("button", { name: "Markdown" }).click();
  await dialog.getByRole("button", { name: "Enviar escolha" }).click();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/tasks/${task.id}/interactions`,
        { headers: auth.headers },
      );
      const pending = (await response.json()) as unknown[];
      return pending.length;
    })
    .toBe(0);

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/tasks/${task.id}`,
        { headers: auth.headers },
      );
      const current = (await response.json()) as { status: string };
      return current.status;
    })
    .toBe("IN_PROGRESS");
});
