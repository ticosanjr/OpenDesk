import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("search shows recent activity before the user types", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Search Recent");
  const auth = api();
  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Última atividade visível",
        description: "Aparece na busca sem digitar nada.",
      },
    },
  );
  expect(created.ok()).toBeTruthy();

  await page.goto("/#/search");
  await expect(page.getByText("Últimas atividades")).toBeVisible();
  await expect(page.getByText("Última atividade visível").first()).toBeVisible();
});
