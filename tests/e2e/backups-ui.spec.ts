import { existsSync, writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("workspace backup can be created and restored as a new workspace", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Backup UI");
  const auth = api();
  writeFileSync(`${workspace.root}/input/brief.md`, "conteúdo do backup");

  await page.goto("/#/settings?section=backups");
  await expect(page.getByText("Backup do workspace")).toBeVisible();
  await page.getByRole("button", { name: "Criar backup" }).click();
  await expect(page.getByText(/backup\(s\)/).first()).toBeVisible();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/backups?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const list = (await response.json()) as Array<{ id: string; files: number }>;
      return list.length;
    })
    .toBe(1);
  const list = await request.get(
    `${auth.baseURL}/api/backups?workspace_id=${workspace.id}`,
    { headers: auth.headers },
  );
  const backups = (await list.json()) as Array<{ id: string; files: number }>;
  expect(backups[0].files).toBeGreaterThan(0);

  await page.getByRole("button", { name: "Restaurar" }).first().click();
  const dialog = page.getByRole("dialog");
  const target = `${workspace.root}-restored`;
  await dialog.getByLabel("Pasta de destino").fill(target);
  await dialog.getByRole("button", { name: "Restaurar", exact: true }).click();
  await expect(page.getByText("Workspace restaurado").first()).toBeVisible();

  expect(existsSync(`${target}/input/brief.md`)).toBe(true);
  await expect
    .poll(async () => {
      const response = await request.get(`${auth.baseURL}/api/workspaces`, {
        headers: auth.headers,
      });
      const workspaces = (await response.json()) as Array<{ root_path: string }>;
      return workspaces.some((item) => item.root_path === target);
    })
    .toBe(true);
});
