import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("sensitive API calls appear in the audit page", async ({ page, request }) => {
  const workspace = await seedWorkspace(request, "Audit UI");
  const auth = api();
  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Tarefa auditada",
        description: "gera um registro de auditoria",
        execution_mode: "BUILD_DIRECT",
      },
    },
  );
  expect(created.ok()).toBeTruthy();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/audit?workspace_id=${workspace.id}&limit=50`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as { entries: Array<{ action: string }> };
      return body.entries.some((entry) => entry.action === "task:POST");
    })
    .toBe(true);

  await page.goto("/#/settings?section=audit");
  await expect(page.getByText("Auditoria da API local")).toBeVisible();
  await expect(page.getByText("task:POST").first()).toBeVisible();
  await expect(page.getByText(/sensível \d+\/min/)).toBeVisible();
});
