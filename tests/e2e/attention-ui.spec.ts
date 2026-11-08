import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("attention dismisses an alert and removes a blocked task", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Attention UI");

  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Aprovação UI E2E",
        description: "Teste de dismiss.",
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
  const attention = page.getByTestId("attention-card");
  await expect(attention.getByText("Aprovação UI E2E")).toBeVisible();
  await attention
    .getByRole("button", { name: "Dispensar alerta de aprovação" })
    .click();
  await expect(attention.getByText("Aprovação UI E2E")).toHaveCount(0);
  await page.reload();
  await expect(page.getByTestId("attention-card").getByText("Aprovação UI E2E")).toHaveCount(0);

  // A new revision makes the alert relevant again.
  await request.post(`${auth.baseURL}/api/tasks/${task.id}/transition`, {
    headers: auth.headers,
    data: { status: "IN_REVIEW" },
  });
  await page.reload();
  await expect(
    page.getByTestId("attention-card").getByText("Aprovação UI E2E"),
  ).toBeVisible();

  const blocked = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Bloqueada UI E2E",
        description: "Teste de remoção.",
        execution_mode: "AUTO_DSH",
        tags: [],
      },
    },
  );
  const blockedTask = (await blocked.json()) as { id: string };
  await request.post(`${auth.baseURL}/api/tasks/${blockedTask.id}/transition`, {
    headers: auth.headers,
    data: { status: "BLOCKED" },
  });
  await page.reload();
  await expect(
    page.getByTestId("attention-card").getByText("Bloqueada UI E2E"),
  ).toBeVisible();
  await page
    .getByTestId("attention-card")
    .getByRole("button", { name: "Remover tarefa" })
    .click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Remover tarefa" })
    .click();
  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/tasks/${blockedTask.id}`,
        { headers: auth.headers },
      );
      return response.status();
    })
    .toBe(404);

});
