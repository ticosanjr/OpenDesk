import { expect, test } from "@playwright/test";
import { seedCompany, seedOperational } from "./fixtures/company";
import { api } from "./helpers";

test("artifact can be opened and pinned from the provenance page", async ({
  page,
  request,
}) => {
  const company = await seedCompany(request, "Artifacts UI");
  const seeded = await seedOperational(request, company);
  await page.goto("/#/artifacts");
  await expect(page.getByText("relatorio-executivo.md").first()).toBeVisible();
  await page.getByTitle("Fixar como entregável").first().click();
  await expect(page.getByText("Artefato fixado").first()).toBeVisible();

  const auth = api();
  const pinned = await request.get(
    `${auth.baseURL}/api/artifacts/pinned?workspace_id=${company.workspace.id}`,
    { headers: auth.headers },
  );
  expect(pinned.ok()).toBeTruthy();
  const list = (await pinned.json()) as Array<{ id: string }>;
  expect(list.some((artifact) => artifact.id === seeded.artifactIds?.[0])).toBe(true);
});

test("artifact viewer opens in the shared modal and closes with Escape", async ({
  page,
  request,
}) => {
  const company = await seedCompany(request, "Artifacts Viewer");
  await seedOperational(request, company);
  await page.goto("/#/artifacts");
  await page.getByText("relatorio-executivo.md").first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("relatorio-executivo.md");
  await expect(dialog.getByRole("button", { name: "Fixar" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
});
