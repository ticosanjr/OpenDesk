import { expect, test } from "@playwright/test";
import { api } from "./helpers";


test("installs and removes a starter pack from the packs page", async ({
  page,
  request,
}) => {
  const auth = api();
  const created = await request.post(`${auth.baseURL}/api/workspaces`, {
    headers: auth.headers,
    data: {
      name: `Packs E2E ${Date.now()}`,
      root_path: `/tmp/opendesk-e2e/workspaces-packs-${Date.now()}`,
    },
  });
  expect(created.ok()).toBeTruthy();
  const workspaceId = ((await created.json()) as { id: string }).id;
  const activated = await request.post(
    `${auth.baseURL}/api/workspaces/${workspaceId}/activate`,
    { headers: auth.headers },
  );
  expect(activated.ok()).toBeTruthy();

  await page.goto("/#/packs");
  await expect(page.getByRole("heading", { name: "Pacotes" })).toBeVisible();
  await page.getByRole("button", { name: /Essenciais/ }).first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("Essenciais", { exact: true }).first()).toBeVisible();
  await dialog.getByRole("button", { name: /Adicionar ao workspace/ }).click();
  await expect(page.getByText("Essenciais adicionado").first()).toBeVisible();

  const agents = (await (
    await request.get(
      `${auth.baseURL}/api/studio/agents?scope=workspace&workspace_id=${workspaceId}`,
      { headers: auth.headers },
    )
  ).json()) as Array<{ id: string }>;
  expect(agents.some((agent) => agent.id === "assistente")).toBe(true);

  const removed = await request.delete(
    `${auth.baseURL}/api/starter-packs/essenciais?workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  expect(removed.ok()).toBeTruthy();
  await request.delete(`${auth.baseURL}/api/workspaces/${workspaceId}`, {
    headers: auth.headers,
  });
});
