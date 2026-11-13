import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("packs can be favorited and the filter survives a reload", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Packs Favorites");
  await page.goto("/#/packs");
  await expect(page.getByText("Essenciais", { exact: true }).first()).toBeVisible();

  await page.getByRole("button", { name: "Favoritar Essenciais" }).click();
  await page.getByRole("button", { name: "Favoritos", exact: true }).click();
  await expect(page.getByText("Essenciais", { exact: true }).first()).toBeVisible();

  await page.reload();
  await page.getByRole("button", { name: "Favoritos", exact: true }).click();
  await expect(page.getByText("Essenciais", { exact: true }).first()).toBeVisible();
  await page
    .getByRole("button", { name: "Remover Essenciais dos favoritos" })
    .click();
});

test("recommended skills list, favorite and install through a task", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Packs Recommended");

  await page.goto("/#/packs");
  await page.getByRole("button", { name: "Recomendados" }).click();
  await expect(page.getByText("Caveman", { exact: true })).toBeVisible();
  await expect(page.getByText("Ponytail", { exact: true })).toBeVisible();
  await expect(page.getByText("Implement", { exact: true })).toBeVisible();
  await expect(page.getByText("Grill Me", { exact: true })).toBeVisible();
  await expect(
    page.getByText("npx skills add mattpocock/skills --skill implement"),
  ).toBeVisible();

  await page.getByRole("button", { name: "Favoritar Caveman" }).click();
  await page.getByRole("button", { name: "Só favoritos" }).click();
  await expect(page.getByText("Caveman", { exact: true })).toBeVisible();
  await expect(page.getByText("Ponytail", { exact: true })).toHaveCount(0);

  await page.getByRole("button", { name: "Só favoritos" }).click();
  const ponytail = page
    .getByText("Ponytail", { exact: true })
    .locator("xpath=ancestor::article[1]");
  await ponytail.getByRole("button", { name: "Instalar via tarefa" }).click();
  await expect(
    page.getByText("Instalação enviada para execução"),
  ).toBeVisible();

  const auth = (await import("./helpers")).api();
  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const tasks = (await response.json()) as Array<{ title: string; description: string }>;
      return tasks.some(
        (task) =>
          task.title === "Instalar skill Ponytail" &&
          task.description.includes("Ponytail"),
      );
    })
    .toBe(true);
});
