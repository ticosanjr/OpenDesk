import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

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
