import { expect, test } from "@playwright/test";
import { existsSync, readFileSync } from "node:fs";
import { seedWorkspace } from "./helpers";

test("new workspace installs the control pack and both MCP bridges", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Control MCP");

  await expect
    .poll(() =>
      existsSync(`${workspace.root}/.opencode/skills/operacao-opendesk/SKILL.md`),
    )
    .toBe(true);
  await expect
    .poll(() => existsSync(`${workspace.root}/.opencode/commands/opendesk-tarefa.md`))
    .toBe(true);
  const config = readFileSync(`${workspace.root}/opencode.jsonc`, "utf8");
  expect(config).toContain("opendesk-config");
  expect(config).toContain("opendesk-control");
  expect(config).toContain("mcp-control");

  await page.goto("/#/studio");
  await page
    .getByRole("main")
    .getByRole("button", { name: /^Workspace ·/ })
    .click();
  await page.getByRole("button", { name: "MCP", exact: true }).click();
  await expect(page.getByText("opendesk-control").first()).toBeVisible();
  await expect(page.getByText("opendesk-config").first()).toBeVisible();

  await page.goto("/#/packs");
  await page.getByRole("button", { name: /Pacotes \(/ }).click();
  await expect(page.getByText("Controle OpenDesk").first()).toBeVisible();
});
