import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("search groups results by type on the all tab", async ({ page, request }) => {
  const workspace = await seedWorkspace(request, "Search UI");
  const auth = api();
  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Relatório semanal de vendas",
        description: "Consolide os números da semana.",
        tags: ["relatorio"],
      },
    },
  );
  expect(created.ok()).toBeTruthy();

  await page.goto("/#/search?q=relat%C3%B3rio");
  await expect(page.getByRole("heading", { name: "Tarefas" })).toBeVisible();
  await expect(
    page.getByText("Relatório semanal de vendas").first(),
  ).toBeVisible();
});
