import { expect, test } from "@playwright/test";
import { seedCompany, seedOperational } from "./fixtures/company";

test("dashboard shows KPIs, recent run and artifact from a real workspace", async ({
  page,
  request,
}) => {
  const company = await seedCompany(request, "Dashboard Data");
  const seeded = await seedOperational(request, company);
  await page.goto("/#/");
  await expect(
    page.getByRole("heading", { name: "Dashboard" }).first(),
  ).toBeVisible();
  await expect(
    page.getByText("Entrega do diagnóstico").first(),
  ).toBeVisible();
  await expect(page.getByText("relatorio-executivo.md").first()).toBeVisible();
  await expect(page.getByText(/runs/).first()).toBeVisible();
  expect(seeded.runId).toBeTruthy();
});
