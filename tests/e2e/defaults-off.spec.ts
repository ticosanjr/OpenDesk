import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("new routines and recipes start disabled until explicitly activated", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Defaults Off");

  await page.goto("/#/routines");
  await page.getByRole("main").getByRole("button", { name: "Nova rotina" }).click();
  const routineDialog = page.getByRole("dialog");
  await routineDialog.getByLabel("Nome da rotina").fill("Rotina desligada");
  await routineDialog.getByLabel("Tipo de gatilho").selectOption("FOLDER_WATCH");
  await routineDialog
    .getByPlaceholder("input/dropzone ou caminho absoluto")
    .fill("input");
  await routineDialog
    .getByPlaceholder("Digite / para comandos e skills, ou descreva a tarefa")
    .fill("Resuma os novos arquivos");
  await routineDialog.getByRole("button", { name: "Criar rotina" }).click();
  await expect(routineDialog).toBeHidden();
  await expect(
    page.getByRole("switch", { name: "Ativar rotina Rotina desligada" }),
  ).toBeVisible();

  // Activation is explicit and can be cancelled.
  await page
    .getByRole("switch", { name: "Ativar rotina Rotina desligada" })
    .click();
  const activation = page.getByRole("dialog");
  await expect(activation).toBeVisible();
  await activation.getByRole("button", { name: "Cancelar", exact: true }).click();
  await expect(
    page.getByRole("switch", { name: "Ativar rotina Rotina desligada" }),
  ).toBeVisible();

  const schedules = (await (
    await request.get(`${auth.baseURL}/api/schedules?workspace_id=${workspace.id}`, {
      headers: auth.headers,
    })
  ).json()) as Array<{ name: string; is_active: boolean }>;
  expect(
    schedules.find((schedule) => schedule.name === "Rotina desligada")?.is_active,
  ).toBe(false);

  await page.goto("/#/recipes");
  await page.getByRole("button", { name: /Nova receita/ }).click();
  await expect(page.getByText("Receita criada").first()).toBeVisible();
  await expect(
    page.getByRole("switch", { name: "Ativar receita Nova receita" }),
  ).toBeVisible();

  const recipes = (await (
    await request.get(`${auth.baseURL}/api/recipes?workspace_id=${workspace.id}`, {
      headers: auth.headers,
    })
  ).json()) as { recipes: Array<{ name: string; is_enabled: boolean }> };
  expect(
    recipes.recipes.find((recipe) => recipe.name === "Nova receita")?.is_enabled,
  ).toBe(false);
});
