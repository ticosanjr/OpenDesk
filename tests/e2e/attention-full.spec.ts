import { expect, test } from "@playwright/test";
import { seedCompany, seedOperational } from "./fixtures/company";
import { api } from "./helpers";

test("attention card explains HITL, permissions and proposals with clear actions", async ({
  page,
  request,
}) => {
  const company = await seedCompany(request, "Attention Full");
  await seedOperational(request, company, {
    hitl: true,
    proposal: true,
    permission: true,
  });
  const auth = api();

  await page.goto("/#/");
  await expect(page.getByText("Precisa da sua atenção").first()).toBeVisible();
  await expect(
    page.getByText("Aprovar o envio da proposta ao cliente?").first(),
  ).toBeVisible();
  await expect(page.getByText("Se aprovar").first()).toBeVisible();
  await expect(page.getByText("Se expirar").first()).toBeVisible();

  await page.getByRole("button", { name: "Aprovar e continuar" }).click();
  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/attention?workspace_id=${company.workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as { hitl: unknown[] };
      return body.hitl.length;
    })
    .toBe(0);

  await page.getByRole("button", { name: "Permitir uma vez" }).click();
  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/attention?workspace_id=${company.workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as { requests: unknown[] };
      return body.requests.length;
    })
    .toBe(0);

  await page.getByText("Propostas de configuração").first().scrollIntoViewIfNeeded();
  await page.getByRole("button", { name: "Aprovar", exact: true }).click();
  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/attention?workspace_id=${company.workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as { proposals: unknown[] };
      return body.proposals.length;
    })
    .toBe(0);
});
