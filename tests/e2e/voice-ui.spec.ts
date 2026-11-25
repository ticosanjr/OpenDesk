import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("voice settings expose TTS/STT tests and task dictation buttons", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Voice UI");

  await page.goto("/#/settings?section=voice");
  await expect(page.getByText("Voz (STT/TTS)")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Testar fala (TTS)" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Testar transcrição (STT)" }),
  ).toBeVisible();
  await expect(page.getByLabel("Provedores STT (ordem)")).toBeVisible();

  await page.goto("/#/kanban?view=board");
  await page.getByRole("button", { name: "Nova tarefa" }).first().click();
  await expect(page.getByRole("button", { name: "Ditar campo principal" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Ditar prompt" })).toBeVisible();
});
