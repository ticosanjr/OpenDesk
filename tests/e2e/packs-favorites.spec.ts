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

test("recommended skills list, favorite and install with preselected skill", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Packs Recommended");
  await page.route("**/api/studio/install/preview", (route) =>
    route.fulfill({
      json: {
        items: [
          {
            kind: "skill",
            id: "ponytail",
            name: "Ponytail",
            description: "Menor mudança que resolve.",
            body: "# Ponytail",
            files: [],
            server: null,
          },
        ],
        warnings: [],
      },
    }),
  );
  await page.route("**/api/studio/install/apply", (route) =>
    route.fulfill({
      json: { ok: true, scope: "workspace", items: [{ kind: "skill", id: "ponytail" }] },
    }),
  );

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
  await page.getByRole("button", { name: "Favoritos", exact: true }).click();
  await expect(page.getByText("Caveman", { exact: true })).toBeVisible();
  await expect(page.getByText("Ponytail", { exact: true })).toHaveCount(0);

  const ponytail = page
    .getByText("Ponytail", { exact: true })
    .locator("xpath=ancestor::article[1]");
  await page.getByRole("button", { name: "Favoritos", exact: true }).click();
  await ponytail.getByRole("button", { name: "Instalar" }).click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("URL direta");
  await expect(dialog).toContainText("ponytail");
  await dialog.getByRole("button", { name: "Instalar selecionados" }).click();
  await expect(page.getByText("Ponytail instalada")).toBeVisible();
});
