import { expect, test } from "@playwright/test";
import { seedCompany, seedOperational } from "./fixtures/company";

test("observability shows latency, failures and agent performance from runs", async ({
  page,
  request,
}) => {
  const company = await seedCompany(request, "Observability Data");
  await seedOperational(request, company, {
    failedRun: false,
    artifacts: false,
    hitl: false,
    proposal: false,
    permission: false,
  });
  await seedOperational(request, company, {
    failedRun: true,
    artifacts: false,
    hitl: false,
    proposal: false,
    permission: false,
  });

  await page.goto("/#/observability");
  await expect(page.getByText("Taxa de sucesso").first()).toBeVisible();
  await expect(page.getByText("Duração p95").first()).toBeVisible();
  await expect(page.getByText("Falhas por código").first()).toBeVisible();
  await expect(page.getByText("timeout").first()).toBeVisible();
  await expect(page.getByText("@build").first()).toBeVisible();
  await expect(page.getByText("Execuções por dia").first()).toBeVisible();
});
