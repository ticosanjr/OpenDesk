import { existsSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("browse integration activates and marks web tasks", async ({ page, request }) => {
  const workspace = await seedWorkspace(request, "Browse UI");
  const auth = api();

  await page.goto("/#/integrations?section=browser");
  await expect(page.getByText("OpenCode Browse").first()).toBeVisible();
  await page.getByRole("button", { name: "Ativar Browse" }).click();
  await expect(page.getByText("MCP ativo").first()).toBeVisible();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/browse/status?workspace_id=${workspace.id}&scope=workspace`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as { configured: boolean; skill: boolean };
      return { configured: body.configured, skill: body.skill };
    })
    .toEqual({ configured: true, skill: true });

  expect(
    existsSync(`${workspace.root}/.opencode/skills/browse-web/SKILL.md`),
  ).toBe(true);

  await page.goto("/#/kanban");
  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Título").fill("Pesquisar concorrentes");
  await dialog
    .getByLabel("Comando ou prompt")
    .fill("Abra https://example.com e compare os preços.");
  await dialog.getByRole("button", { name: /Mais opções/ }).click();
  const web = dialog.getByRole("switch", {
    name: "Precisa navegar na internet",
  });
  await expect(web).toHaveAttribute("aria-checked", "false");
  await web.click();
  await expect(web).toHaveAttribute("aria-checked", "true");
  await dialog.getByRole("button", { name: /Criar tarefa/ }).click();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const tasks = (await response.json()) as Array<{
        title: string;
        tags_json: string;
      }>;
      const task = tasks.find((item) => item.title === "Pesquisar concorrentes");
      return task?.tags_json ?? "";
    })
    .toContain("needs-browser");
});
