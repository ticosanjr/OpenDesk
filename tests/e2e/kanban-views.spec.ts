import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("kanban saves and reapplies a named filter view", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Kanban Views");
  const auth = api();
  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Tarefa do build",
        description: "Filtro salvo.",
        assigned_agent: "build",
        priority: 3,
        tags: [],
      },
    },
  );
  expect(created.ok()).toBeTruthy();

  await page.goto("/#/kanban");
  await page.getByRole("button", { name: /Filtros/ }).click();
  await page.getByLabel("Filtrar por agente").selectOption("build");
  await page.getByLabel("Filtrar por prioridade").selectOption("3");
  await page.getByLabel("Nome da visão").fill("Build alta");
  await page.getByRole("button", { name: "Salvar", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByText("Filtros ativos:")).toBeVisible();

  await page.reload();
  await page.getByRole("button", { name: /Filtros/ }).click();
  await page.getByRole("button", { name: "Build alta", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Remover filtro do agente @build" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Remover filtro de prioridade alta" }),
  ).toBeVisible();
  await expect(page.getByText("Tarefa do build")).toBeVisible();

  await page.getByRole("button", { name: /Filtros/ }).click();
  await page.getByRole("button", { name: "Remover visão Build alta" }).click();
});
