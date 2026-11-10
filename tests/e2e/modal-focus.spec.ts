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
