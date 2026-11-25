import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("backup zip can be imported from the settings card", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Backup Import");
  const auth = api();
  writeFileSync(`${workspace.root}/input/brief.md`, "conteúdo importado");

  const created = await request.post(
    `${auth.baseURL}/api/backups?workspace_id=${workspace.id}`,
    { headers: auth.headers },
  );
  expect(created.ok()).toBeTruthy();
  const backup = (await created.json()) as { id: string; file_name: string };
  const download = await request.get(
    `${auth.baseURL}/api/backups/${backup.id}/download?workspace_id=${workspace.id}`,
    { headers: auth.headers },
  );
  expect(download.ok()).toBeTruthy();
  const zipPath = `/tmp/opencode/${backup.file_name}`;
  writeFileSync(zipPath, await download.body());

  await page.goto("/#/settings?section=backups");
  await expect(page.getByText("Backup do workspace")).toBeVisible();
  await expect(page.getByText(`${backup.file_name}`)).toBeVisible();

  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Importar .zip" }).click();
  await (await chooserPromise).setFiles(zipPath);
  await expect(page.getByText("Backup importado")).toBeVisible();
  await expect(page.getByText(/^imported-/).first()).toBeVisible();
});
