import { expect, test } from "@playwright/test";
import { seedCompany } from "./fixtures/company";
import { startFakeSmtp, type FakeSmtp } from "./fixtures/smtp";

let smtp: FakeSmtp;

test.beforeAll(async () => {
  smtp = await startFakeSmtp();
});

test.afterAll(async () => {
  await smtp.close();
});

test("SMTP setup tests a real connection and integration tabs render", async ({
  page,
  request,
}) => {
  await seedCompany(request, "Integrations SMTP");
  await page.goto("/#/integrations?section=smtp");
  await expect(page.getByRole("tab", { name: "SMTP" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "SMTP" })).toBeVisible();
  await page.getByLabel("Host").fill("127.0.0.1");
  await page.getByLabel("Porta").fill(String(smtp.port));
  await page.getByLabel("Remetente").fill("OpenDesk <teste@example.com>");
  const tls = page.getByLabel("usar TLS");
  if (await tls.isChecked()) await tls.uncheck();
  await page.getByRole("button", { name: "Salvar", exact: true }).click();
  await expect(page.getByText("SMTP salvo.").first()).toBeVisible();
  await page.getByRole("button", { name: "Testar", exact: true }).click();
  await expect(page.getByText("Conexão SMTP ok.").first()).toBeVisible();

  await page.goto("/#/integrations?section=google");
  await expect(page.getByRole("tab", { name: "Google" })).toBeVisible();
  await page.goto("/#/integrations?section=signal");
  await expect(page.getByRole("tab", { name: "Signal" })).toBeVisible();
  await page.goto("/#/integrations?section=inbound");
  await expect(page.getByRole("tab", { name: "Entradas" })).toBeVisible();
  await page.goto("/#/integrations?section=sinks");
  await expect(page.getByRole("tab", { name: "Saídas" })).toBeVisible();
});
