import { expect, test } from "@playwright/test";
import { api } from "./helpers";


test("attention card, task fork and workspace memory", async ({ page, request }) => {
  const auth = api();
  const existing = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  let workspaceId = existing[0]?.id;
  if (!workspaceId) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: { name: "Attention Workspace", root_path: "/tmp/opendesk-e2e/workspaces-attention" },
    });
    workspaceId = ((await created.json()) as { id: string }).id;
  }

  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        title: "Precisa de aprovação E2E",
        description: "Plano aguardando aprovação.",
        execution_mode: "PLAN_THEN_BUILD",
        tags: [],
      },
    },
  );
  expect(created.ok()).toBeTruthy();
  const task = (await created.json()) as { id: string };
  await request.post(`${auth.baseURL}/api/tasks/${task.id}/transition`, {
    headers: auth.headers,
    data: { status: "IN_REVIEW" },
  });

  await page.goto("/#/");
  await expect(page.getByText("Precisa da sua atenção")).toBeVisible();
  await expect(page.getByText("plano pronto para revisão")).toBeVisible();

  const forked = await request.post(`${auth.baseURL}/api/tasks/${task.id}/fork`, {
    headers: auth.headers,
  });
  expect(forked.ok()).toBeTruthy();
  const fork = (await forked.json()) as { title: string; forked_from_task_id: string };
  expect(fork.title).toContain("(fork)");
  expect(fork.forked_from_task_id).toBe(task.id);

  const memory = await request.post(`${auth.baseURL}/api/tasks/${task.id}/memory`, {
    headers: auth.headers,
    data: { content: "Sempre validar o build antes de concluir." },
  });
  expect(memory.ok()).toBeTruthy();
  const loaded = await request.get(`${auth.baseURL}/api/tasks/${task.id}/memory`, {
    headers: auth.headers,
  });
  expect(await loaded.text()).toContain("Sempre validar o build");
});
