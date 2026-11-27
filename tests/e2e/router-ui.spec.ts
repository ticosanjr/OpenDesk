import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("router lets the user save the cost blend", async ({ page, request }) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Router UI");

  await page.goto("/#/router");
  await expect(page.getByRole("heading", { name: "Roteador & Custos" })).toBeVisible();
  await page.getByRole("tab", { name: "Análise de Custos" }).click();
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

test("router saves OpenDesk models and tests every policy", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Router Policy");
  await page.goto("/#/router");
  await expect(page.getByRole("heading", { name: "Roteador & Custos" })).toBeVisible();

  const defaultModel = page.getByRole("combobox", { name: "Modelo padrão" });
  await defaultModel.fill("deepseek/deepseek-flash");
  await defaultModel.press("Enter");
  const manualModel = page.getByRole("combobox", { name: "Modelo do modo manual" });
  await manualModel.fill("deepseek/deepseek-v4-pro");
  await manualModel.press("Enter");
  await page.getByRole("button", { name: /Salvar modelos/ }).click();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/router/config?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as {
        default_model?: string | null;
        manual_model?: string | null;
      };
      return `${body.default_model ?? ""}|${body.manual_model ?? ""}`;
    })
    .toBe("deepseek/deepseek-flash|deepseek/deepseek-v4-pro");

  await page.getByRole("button", { name: /Testar todas/ }).click();
  await expect(page.getByText("deepseek/deepseek-flash").first()).toBeVisible();
  await expect(page.getByText("deepseek/deepseek-v4-pro").first()).toBeVisible();
  await expect(page.getByText("Regras customizadas").first()).toBeVisible();

  await page.getByRole("button", { name: "Testar modelo" }).first().click();
  await expect(page.getByText(/engine_offline|OK ·/).first()).toBeVisible();

  await page.getByRole("button", { name: "Regras customizadas" }).click();
  await page.getByRole("tab", { name: "Regras", exact: true }).click();
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

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/router/rules?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const rules = (await response.json()) as Array<{ name: string }>;
      return rules.some((rule) => rule.name === "Regra E2E editada");
    })
    .toBe(true);

  await page.getByRole("tab", { name: "Políticas & Modelos" }).click();
  await page.getByRole("button", { name: "Melhor custo-benefício" }).click();
  await page.getByRole("tab", { name: "Análise de Custos" }).click();
  await expect(page.getByText("Modelo roteador e comparação")).toBeVisible();
});
