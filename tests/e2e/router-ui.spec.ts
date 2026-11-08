import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("router lets the user save the cost blend", async ({ page, request }) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Router UI");

  await page.goto("/#/router");
  await expect(page.getByRole("heading", { name: "Roteador & Custos" })).toBeVisible();
  const range = page.getByLabel("Peso da entrada");
  await range.focus();
  await range.press("Home");
  for (let index = 0; index < 10; index += 1) {
    await range.press("ArrowRight");
  }
  await page.getByRole("button", { name: /Salvar blend/ }).click();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/router/config?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as { cost_blend: number };
      return body.cost_blend;
    })
    .toBe(0.5);

});
