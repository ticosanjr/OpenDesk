import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";
import { seedCompany, seedOperational } from "./fixtures/company";

test("typing in a modal keeps focus and does not close it", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Modal Focus");

  await page.goto("/#/kanban");
  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  const dialog = page.getByRole("dialog");
  const title = dialog.getByPlaceholder("Relatório executivo de vendas");
  await title.click();
  // pressSequentially reproduces real typing. Regression: the old modal
  // refocused the close button after the first keystroke, so the space closed
  // the dialog and the remaining text was lost.
  await title.pressSequentially("Tarefa com espaço", { delay: 10 });

  await expect(title).toHaveValue("Tarefa com espaço");
  await expect(title).toBeFocused();
  await expect(dialog).toBeVisible();
});

test("workspace modal accepts character-by-character typing", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Modal Workspace");
  await page.goto("/#/");
  await page.getByRole("button", { name: "Novo workspace" }).click();
  const dialog = page.getByRole("dialog");
  const name = dialog.getByLabel("Nome", { exact: true });
  await name.click();
  await name.pressSequentially("Workspace digitado", { delay: 5 });
  await expect(name).toHaveValue("Workspace digitado");
  await expect(name).toBeFocused();
  await expect(dialog).toBeVisible();
});

test("calendar event modal accepts character-by-character typing", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Modal Calendar");
  await page.goto("/#/calendar");
  await page.getByRole("main").getByRole("button", { name: "Novo evento" }).click();
  const dialog = page.getByRole("dialog");
  const title = dialog.getByLabel("Título", { exact: true });
  await title.click();
  await title.pressSequentially("Evento com espaço", { delay: 5 });
  await expect(title).toHaveValue("Evento com espaço");
  await expect(title).toBeFocused();
  await expect(dialog).toBeVisible();
});

test("routine modal accepts character-by-character typing", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Modal Routine");
  await page.goto("/#/routines");
  await page.getByRole("main").getByRole("button", { name: "Nova rotina" }).click();
  const dialog = page.getByRole("dialog");
  const name = dialog.getByLabel("Nome da rotina");
  await name.click();
  await name.pressSequentially("Rotina digitada", { delay: 5 });
  await expect(name).toHaveValue("Rotina digitada");
  await expect(name).toBeFocused();
  await expect(dialog).toBeVisible();
});

test("modals are centered and occupy a large part of the viewport", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Modal Layout");
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/#/kanban");
  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  const box = await dialog.boundingBox();
  expect(box).not.toBeNull();
  if (!box) return;
  expect(box.width).toBeGreaterThan(900);
  expect(box.height).toBeGreaterThan(440);
  const centerX = box.x + box.width / 2;
  const centerY = box.y + box.height / 2;
  expect(Math.abs(centerX - 720)).toBeLessThan(8);
  expect(Math.abs(centerY - 450)).toBeLessThan(8);
});

test("execution preview is centered and occupies a large part of the viewport", async ({
  page,
  request,
}) => {
  const company = await seedCompany(request, "Execution Preview");
  await seedOperational(request, company);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/#/kanban");
  await page.getByText("Entrega do diagnóstico").first().click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Execução", exact: true })
    .click();
  const drawer = page.locator("aside").filter({ hasText: "abrir página" });
  await expect(drawer).toBeVisible();
  const box = await drawer.boundingBox();
  expect(box).not.toBeNull();
  if (!box) return;
  expect(box.width).toBeGreaterThan(900);
  expect(box.height).toBeGreaterThan(500);
  expect(Math.abs(box.x + box.width / 2 - 720)).toBeLessThan(8);
  expect(Math.abs(box.y + box.height / 2 - 450)).toBeLessThan(8);
  await page.keyboard.press("Escape");
  await expect(drawer).toBeHidden();
});

test("typing a question mark in a field does not open global shortcuts", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Shortcut Guard");
  await page.goto("/#/kanban");
  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  const title = page
    .getByRole("dialog")
    .getByPlaceholder("Relatório executivo de vendas");
  await title.click();
  await title.pressSequentially("Teste? com interrogação", { delay: 5 });
  await expect(title).toHaveValue("Teste? com interrogação");
  await expect(page.getByText("Atalhos do OpenDesk")).toHaveCount(0);
  await page.keyboard.press("Escape");

  await page.getByRole("heading", { name: "Kanban" }).click();
  await page.keyboard.press("?");
  await expect(page.getByText("Atalhos do OpenDesk").first()).toBeVisible();
});
