import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("routine accordion accepts folder watch and a recipe target", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Routines UI");
  const recipeId = "receita-rotina-ui";
  const recipeName = "Receita Rotina UI";
  const yaml = `id: ${recipeId}
name: ${recipeName}
enabled: true
triggers: []
flow:
  - type: step
    id: passo-1
    name: Passo 1
    mode: build
    prompt: ok
`;
  const saved = await request.put(
    `${auth.baseURL}/api/recipes/${recipeId}?workspace_id=${workspace.id}`,
    { headers: auth.headers, data: { yaml } },
  );
  expect(saved.ok()).toBeTruthy();

  await page.goto("/#/routines");
  await page.getByRole("main").getByRole("button", { name: "Nova rotina" }).click();
  const dialog = page.getByRole("dialog");
  const timezone = dialog.getByLabel("Fuso horário");
  await expect(timezone).toBeVisible();
  await expect(timezone).toContainText("UTC+00:00");
  await dialog.getByLabel("Nome da rotina").fill("Rotina Receita UI");
  await dialog.getByLabel("Tipo de gatilho").selectOption("FOLDER_WATCH");
  await dialog
    .getByPlaceholder("input/dropzone ou caminho absoluto")
    .fill("input");
  await dialog.getByRole("button", { name: "Receita", exact: true }).click();
  await dialog
    .getByPlaceholder("Digite para buscar a receita…")
    .fill("Receita Rotina");
  await dialog.getByRole("button", { name: /Receita Rotina UI/ }).click();
  await dialog.getByRole("button", { name: "Criar rotina" }).click();
  await expect(dialog).toBeHidden();

  await page.getByRole("button", { name: /Mais ações da rotina/ }).first().click();
  await expect(page.getByRole("button", { name: "Histórico" })).toBeVisible();
  await page.getByRole("heading", { name: "Rotinas & Gatilhos" }).click();
  await expect(page.getByRole("button", { name: "Histórico" })).toBeHidden();

  await expect
    .poll(async () => {
      const response = await request.get(`${auth.baseURL}/api/schedules`, {
        headers: auth.headers,
      });
      const body = (await response.json()) as Array<{
        id: string;
        name: string;
        trigger_type: string;
        target_kind: string;
        recipe_id: string | null;
      }>;
      const schedule = body.find((item) => item.name === "Rotina Receita UI");
      if (!schedule) return false;
      expect(schedule.trigger_type).toBe("FOLDER_WATCH");
      expect(schedule.target_kind).toBe("RECIPE");
      expect(schedule.recipe_id).toBe(recipeId);
      return true;
    })
    .toBe(true);

  const sources = await request.get(
    `${auth.baseURL}/api/io/inbound?workspace_id=${workspace.id}`,
    { headers: auth.headers },
  );
  expect(((await sources.json()) as { sources: unknown[] }).sources).toHaveLength(1);

});
