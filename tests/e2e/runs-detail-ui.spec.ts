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

  const tool = page
    .getByRole("button", { name: /read · input\/brief\.md/ })
    .first();
  await expect(tool).toHaveAttribute("aria-expanded", "false");
  await tool.click();
  await expect(tool).toHaveAttribute("aria-expanded", "true");
  await expect(page.getByText("entrada", { exact: true }).first()).toBeVisible();
  await page.getByText("conteúdo do briefing").first().click();
  await expect(tool).toHaveAttribute("aria-expanded", "false");

  const generic = page
    .getByRole("button", { name: /session\.permission\.requested/ })
    .first();
  await expect(generic).toHaveAttribute("aria-expanded", "false");
  await generic.click();
  await expect(generic).toHaveAttribute("aria-expanded", "true");
  const payload = page.getByText(/"resource": "npm test"/).first();
  await expect(payload).toBeVisible();
  await payload.click();
  await expect(generic).toHaveAttribute("aria-expanded", "false");

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
