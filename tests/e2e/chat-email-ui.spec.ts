import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("email chat thread exposes the reply composer", async ({ page, request }) => {
  const workspace = await seedWorkspace(request, "Chat Email");
  const auth = api();
  const created = await request.post(`${auth.baseURL}/api/chat/threads`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      title: "E-mail · Orçamento",
      agent: "build",
      source: "email",
      external_id: "<root@example.com>",
    },
  });
  expect(created.ok()).toBeTruthy();
  const thread = (await created.json()) as { id: string };

  await page.goto(`/#/chat?thread=${thread.id}`);
  await expect(page.getByText("E-mail · Orçamento")).toBeVisible();
  await page.getByTitle("Responder por e-mail").click();
  await expect(page.getByText("Responder por e-mail")).toBeVisible();
  await expect(page.getByLabel("Para")).toBeVisible();
  await expect(page.getByLabel("Assunto")).toHaveValue("Re: Orçamento");
  await page.getByLabel("Para").fill("cliente@example.com");
  await page.getByLabel("Mensagem").fill("Segue a resposta.");
  await page.getByRole("dialog").getByRole("button", { name: "Enviar" }).click();
  await expect(page.getByText("Envio falhou")).toBeVisible();
});
