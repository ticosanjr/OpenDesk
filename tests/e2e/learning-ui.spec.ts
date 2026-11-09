import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("learning page exposes light capture controls and empty-state scan", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Learning UI");
  const auth = api();

  await page.goto("/#/learning");
  await expect(page.getByRole("heading", { name: "Aprendizado" })).toBeVisible();
  await expect(page.getByText("Captura leve e controlada")).toBeVisible();

  const toggle = page.getByRole("switch", { name: "Ativar aprendizado" });
  await expect(toggle).toHaveAttribute("aria-checked", "true");
  await toggle.click();
  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/learning?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as {
        settings: { enabled: boolean };
      };
      return body.settings.enabled;
    })
    .toBe(false);

  await toggle.click();
  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/learning?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as {
        settings: { enabled: boolean };
      };
      return body.settings.enabled;
    })
    .toBe(true);

  await page.getByRole("button", { name: "Analisar execuções" }).click();
  await expect(
    page.getByText("Nenhuma novidade nas execuções recentes").first(),
  ).toBeVisible();
});
