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

test("router adapts to policy and tests routing/model", async ({ page, request }) => {
  const workspace = await seedWorkspace(request, "Router Policy");
  await page.goto("/#/router");
  await expect(page.getByRole("heading", { name: "Roteador & Custos" })).toBeVisible();

  await page.getByRole("button", { name: "Regras customizadas" }).click();
  await expect(page.getByText("Regras IF-THEN")).toBeVisible();
  await expect(page.getByText("Modelo roteador e comparação")).toBeHidden();

  await page.getByRole("button", { name: "Nova regra" }).click();
  let ruleDialog = page.getByRole("dialog");
  await ruleDialog.getByLabel("Nome").fill("Regra E2E");
  await ruleDialog.getByRole("button", { name: "Criar regra" }).click();
  await expect(ruleDialog).toBeHidden();
  await expect(page.getByText("Regra E2E").first()).toBeVisible();

  await page.getByTitle("Editar regra").first().click();
  ruleDialog = page.getByRole("dialog");
  await expect(ruleDialog.getByText("Editar regra do roteador")).toBeVisible();
  await ruleDialog.getByLabel("Nome").fill("Regra E2E editada");
  await ruleDialog.getByRole("button", { name: "Salvar regra" }).click();
  await expect(ruleDialog).toBeHidden();
  await expect(page.getByText("Regra E2E editada").first()).toBeVisible();

  await page.getByRole("button", { name: "Melhor custo-benefício" }).click();
  await expect(page.getByText("Modelo roteador e comparação")).toBeVisible();
  await page.getByRole("button", { name: /Testar roteamento/ }).click();
  await expect(page.getByText(/@build · opencode-go\//)).toBeVisible();

  await expect
    .poll(async () => {
      const auth = api();
      const response = await request.get(
        `${auth.baseURL}/api/router/rules?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const rules = (await response.json()) as Array<{ name: string }>;
      return rules.some((rule) => rule.name === "Regra E2E editada");
    })
    .toBe(true);

  await page.getByRole("button", { name: "Modelo específico", exact: true }).click();
  const model = page.getByRole("combobox", { name: "Modelo" });
  await model.fill("deepseek/deepseek-flash");
  await model.press("Enter");
  await page.getByRole("button", { name: "Testar modelo", exact: true }).click();
  await expect(page.getByText(/engine_offline/)).toBeVisible();
});
