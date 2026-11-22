import { expect, test, type APIRequestContext } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

/// SQLite is briefly locked by background delivery/cleanup in the full suite;
/// retry the transition and wait until the row really changed before loading
/// the UI.
async function transitionUntil(
  request: APIRequestContext,
  auth: ReturnType<typeof api>,
  taskId: string,
  status: string,
) {
  for (let attempt = 0; attempt < 10; attempt += 1) {
    const response = await request.post(
      `${auth.baseURL}/api/tasks/${taskId}/transition`,
      { headers: auth.headers, data: { status } },
    );
    if (response.ok()) break;
    await new Promise((resolve) => setTimeout(resolve, 200 * (attempt + 1)));
  }
  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/tasks/${taskId}`,
        { headers: auth.headers },
      );
      if (!response.ok()) return response.status();
      return ((await response.json()) as { status: string }).status;
    })
    .toBe(status);
}

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
  await transitionUntil(request, auth, task.id, "IN_REVIEW");

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
  await transitionUntil(request, auth, task.id, "IN_REVIEW");
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
  await transitionUntil(request, auth, blockedTask.id, "BLOCKED");
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
