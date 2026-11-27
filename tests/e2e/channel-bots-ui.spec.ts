import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("workspace bots create isolated group contexts", async ({ page, request }) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Bots UI");
  const token = `123456:abcdefghijklmnopqrstuvwxyz${Date.now()}`;
  const created = await request.post(`${auth.baseURL}/api/channels/bots`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      channel: "telegram",
      name: "Bot E2E",
      enabled: false,
      token,
      allowlist: ["-1001"],
      default_agent: "build",
    },
  });
  expect(created.ok()).toBeTruthy();

  await page.goto("/#/integrations?section=telegram");
  await expect(page.getByText("Bots de Telegram")).toBeVisible();
  await expect(page.getByText(workspace.name).first()).toBeVisible();
  await expect(page.getByText("Bot E2E")).toBeVisible();

  await page.getByRole("button", { name: "Contextos (0)" }).click();
  await expect(page.getByText("Contextos de Bot E2E")).toBeVisible();
  await page.getByRole("button", { name: "Adicionar grupo/chat" }).click();
  await page.getByLabel("ID do chat/grupo").fill("-1001");
  await page.getByLabel("Título amigável").fill("Trabalho");
  await page.getByRole("button", { name: "Adicionar", exact: true }).click();
  await expect(page.getByText("Contextos de Bot E2E")).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Título" })).toHaveValue(
    "Trabalho",
  );

  await page
    .getByLabel("Instruções exclusivas do grupo")
    .fill("Responda como o time de engenharia.");
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Salvar", exact: true })
    .click();
  await expect(page.getByText("Contexto do grupo salvo").first()).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "Contextos (1)" })).toBeVisible();
});
