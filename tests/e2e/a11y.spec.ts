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
