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

test("main routes and the task modal have no critical or serious accessibility violations", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "A11y");

  for (const route of ["/#/", "/#/kanban", "/#/studio", "/#/settings"]) {
    await page.goto(route);
    await page.waitForLoadState("networkidle");
    expect(await criticalViolations(page), route).toEqual([]);
  }

  await page.goto("/#/kanban");
  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await criticalViolations(page), "task modal").toEqual([]);
});
