import { expect, test } from "@playwright/test";
import { seedCompany, seedOperational } from "./fixtures/company";
import { api } from "./helpers";

test("runs list opens detail and exports the session", async ({ page, request }) => {
  const company = await seedCompany(request, "Runs Detail");
  const seeded = await seedOperational(request, company);
  await page.goto("/#/runs");
  await expect(page.getByText("Entrega do diagnóstico").first()).toBeVisible();
  await page.getByText("Entrega do diagnóstico").first().click();
  await expect(page.getByText("Execução", { exact: true }).first()).toBeVisible();
  await expect(page.getByRole("button", { name: "MD", exact: true })).toBeVisible();

  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "MD", exact: true }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toMatch(/\.md$/);

  const auth = api();
  const exportResponse = await request.get(
    `${auth.baseURL}/api/runs/${seeded.runId}/export?format=json`,
    { headers: auth.headers },
  );
  expect(exportResponse.ok()).toBeTruthy();
});
