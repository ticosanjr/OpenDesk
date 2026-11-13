import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { seedWorkspace } from "./helpers";

async function criticalViolations(page: Page) {
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  return results.violations
    .filter((violation) => violation.impact === "critical" || violation.impact === "serious")
    .map((violation) => ({
      id: violation.id,
      help: violation.help,
      nodes: violation.nodes.map((node) => node.target.join(" ")),
    }));
}

test("main routes and key modals have no critical or serious accessibility violations", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "A11y");

  for (const route of [
    "/#/",
    "/#/kanban",
    "/#/studio",
    "/#/settings",
    "/#/chat",
    "/#/calendar",
    "/#/files",
    "/#/artifacts",
    "/#/recipes",
    "/#/routines",
    "/#/runs",
    "/#/observability",
    "/#/learning",
    "/#/governance",
    "/#/integrations",
    "/#/packs",
    "/#/router",
  ]) {
    await page.goto(route);
    await page.waitForLoadState("networkidle");
    expect(await criticalViolations(page), route).toEqual([]);
  }

  await page.goto("/#/kanban");
  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await criticalViolations(page), "task modal").toEqual([]);
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Novo workspace" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await criticalViolations(page), "workspace modal").toEqual([]);
  await page.getByRole("dialog").getByRole("button", { name: "Cancelar" }).click();

  await page.goto("/#/calendar");
  await page.getByRole("main").getByRole("button", { name: "Novo evento" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await criticalViolations(page), "event modal").toEqual([]);
  await page.keyboard.press("Escape");

  await page.goto("/#/routines");
  await page.getByRole("main").getByRole("button", { name: "Nova rotina" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await criticalViolations(page), "routine modal").toEqual([]);
});

test("main routes keep axe clean at compact and wide viewports", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "A11y Viewports");
  for (const viewport of [
    { width: 1024, height: 768 },
    { width: 1920, height: 1080 },
  ]) {
    await page.setViewportSize(viewport);
    for (const route of [
      "/#/",
      "/#/kanban",
      "/#/studio",
      "/#/settings",
      "/#/chat",
      "/#/calendar",
      "/#/files",
      "/#/artifacts",
      "/#/recipes",
      "/#/routines",
      "/#/runs",
      "/#/observability",
      "/#/learning",
      "/#/governance",
      "/#/integrations",
      "/#/packs",
      "/#/router",
    ]) {
      await page.goto(route);
      await page.waitForLoadState("networkidle");
      expect(await criticalViolations(page), `${route} @ ${viewport.width}`).toEqual(
        [],
      );
    }
  }
});

test("install, routine and recipe modals keep axe clean", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "A11y Modals");
  const auth = (await import("./helpers")).api();
  const recipe = await request.put(
    `${auth.baseURL}/api/recipes/a11y-recipe?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        yaml: `id: a11y-recipe
name: Receita A11y
enabled: true
triggers: []
flow:
  - type: step
    id: passo-1
    name: Passo
    mode: build
    prompt: ok
`,
      },
    },
  );
  expect(recipe.ok()).toBeTruthy();

  await page.setViewportSize({ width: 1100, height: 800 });
  await page.goto("/#/packs");
  await page.getByRole("button", { name: /Essenciais/ }).first().click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await criticalViolations(page), "pack install").toEqual([]);
  await page.getByRole("dialog").getByRole("button", { name: "Fechar" }).click();

  await page.goto("/#/routines");
  await page.getByRole("main").getByRole("button", { name: "Nova rotina" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await criticalViolations(page), "routine form").toEqual([]);
  await page.getByRole("dialog").getByRole("button", { name: "Cancelar" }).click();

  await page.goto("/#/recipes");
  await page.getByText("Receita A11y").first().click();
  await page.getByRole("button", { name: /Dry-run/ }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await criticalViolations(page), "recipe run").toEqual([]);
  await page.getByRole("dialog").getByRole("button", { name: "Cancelar" }).click();

  await page.goto("/#/studio");
  await page.getByRole("button", { name: "Instalar" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await criticalViolations(page), "studio install").toEqual([]);
});
