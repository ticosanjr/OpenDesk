import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("workspace dialog offers blank, template and existing modes", async ({ page }) => {
  await page.goto("/#/");
  await page.getByRole("button", { name: "Novo workspace" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("button", { name: "Em branco" })).toBeVisible();
  await dialog.getByRole("button", { name: "Template" }).click();
  await expect(dialog.getByText("Modelo de workspace")).toBeVisible();
  await expect(dialog.getByText("Fábrica de Capacidades")).toBeVisible();
  await dialog.getByRole("button", { name: "Existente" }).click();
  await expect(dialog.getByText("Pasta do workspace existente")).toBeVisible();
  await dialog.getByRole("button", { name: "Cancelar" }).click();
});

test("workspace color drives the theme and kanban retention persists", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Workspace Theme", "#ef4444");

  await page.goto("/#/");
  const scope = page.locator(".workspace-scope");
  await expect(scope).toBeVisible();
  const accent = await scope.evaluate((element) =>
    getComputedStyle(element).getPropertyValue("--workspace-accent").trim(),
  );
  expect(accent.toLowerCase()).toBe("#ef4444");
  const primary = await page
    .getByRole("button", { name: /Nova tarefa/ })
    .first()
    .evaluate((element) => getComputedStyle(element).backgroundColor);
  // The primary button uses 85% of the workspace accent mixed with black so
  // white text keeps WCAG AA contrast.
  const channels = primary.match(/[\d.]+/g)?.map(Number) ?? [];
  const expected = [239, 68, 68].map((value) => (value * 0.85) / 255);
  expect(channels).toHaveLength(3);
  expected.forEach((value, index) =>
    expect(channels[index]).toBeCloseTo(value, 2),
  );

  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Retenção UI E2E",
        description: "Card recente deve continuar visível.",
        execution_mode: "AUTO_DSH",
        tags: [],
      },
    },
  );
  const task = (await created.json()) as { id: string };
  await request.post(`${auth.baseURL}/api/tasks/${task.id}/transition`, {
    headers: auth.headers,
    data: { status: "DONE" },
  });

  await page.goto("/#/kanban");
  await expect(page.getByText("Retenção UI E2E").first()).toBeVisible();
  await page.getByRole("button", { name: /Visibilidade/ }).click();
  await page.getByLabel("Período de retenção").selectOption("1h");
  await page.getByLabel("Máximo de cards concluídos").fill("3");
  await page.getByRole("button", { name: "fechar", exact: true }).click();

  await page.reload();
  await page.getByRole("button", { name: /Visibilidade/ }).click();
  await expect(page.getByLabel("Período de retenção")).toHaveValue("1h");
  await expect(page.getByLabel("Máximo de cards concluídos")).toHaveValue("3");
  await expect(page.getByText("Retenção UI E2E").first()).toBeVisible();

});
