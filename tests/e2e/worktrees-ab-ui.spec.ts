import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("creates an A/B group and opens both slots", async ({ page, request }) => {
  const workspace = await seedWorkspace(request, "Worktrees AB");
  const auth = api();
  const created = await request.post(`${auth.baseURL}/api/runs/ab`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      title: "Comparar parser",
      prompt: "Refatore o parser mantendo os testes verdes.",
      agent_a: "build",
      agent_b: "build",
      model_a: "opencode-go/glm-5.3",
      model_b: "opencode-go/glm-5.3",
      effort: "medium",
    },
  });
  expect(created.ok()).toBeTruthy();

  await page.goto("/#/worktrees");
  await page.getByRole("tab", { name: /A\/B/ }).click();
  await expect(page.getByText("Comparar parser")).toBeVisible();
  await page.getByText("Comparar parser").click();
  await expect(page.getByText("SLOT A")).toBeVisible();
  await expect(page.getByText("SLOT B")).toBeVisible();
  await expect(page.getByRole("button", { name: "Escolher A" })).toBeDisabled();
});
