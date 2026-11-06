import { expect, test } from "@playwright/test";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";

const DATA_DIR = "/tmp/opendesk-e2e/data";

function api() {
  const endpoint = JSON.parse(readFileSync(`${DATA_DIR}/api.json`, "utf8")) as {
    port: number;
    token: string;
  };
  return {
    baseURL: `http://127.0.0.1:${endpoint.port}`,
    headers: { Authorization: `Bearer ${endpoint.token}` },
  };
}

test("workspace, settings, Studio and external edit sync", async ({ page, request }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();

  // Create a workspace under a chosen parent folder.
  await page.getByTitle("Novo workspace").click();
  await page
    .getByPlaceholder("Neuralab, Bernardes AI, Home Lab…")
    .fill("E2E Workspace");
  await page
    .getByPlaceholder("~/Documents/OpenDesk Workspaces")
    .fill("/tmp/opendesk-e2e/workspaces");
  await page.getByRole("button", { name: "Criar workspace" }).click();

  await expect(page.locator("select").first()).toContainText("E2E Workspace");

  // Settings: change the default workspaces folder.
  await page.getByRole("link", { name: "Configurações" }).click();
  await expect(page.getByRole("heading", { name: "Configurações" })).toBeVisible();
  await page
    .getByPlaceholder("~/Documents/OpenDesk Workspaces")
    .fill("/tmp/opendesk-e2e/default-root");
  await page.getByRole("button", { name: "Salvar" }).first().click();
  await expect(page.getByText("salvo")).toBeVisible();

  // Studio: create an agent with name, avatar and color.
  await page.getByRole("link", { name: "OpenCode Studio" }).click();
  await expect(page.getByRole("heading", { name: "OpenCode Studio" })).toBeVisible();
  await page.getByTitle("Novo agente").click();
  await page.locator('input[placeholder="auditor"]').fill("revisor-fiscal");
  await page
    .locator('input[placeholder="Auditor de Qualidade"]')
    .fill("Revisor Fiscal");
  await page.locator('input[placeholder="🛡️"]').fill("🧾");
  await page
    .locator('textarea[placeholder^="Você é o Auditor"]')
    .fill("Você é o revisor fiscal do workspace.");
  await page.getByRole("button", { name: "Salvar" }).click();
  await expect(page.getByText("Revisor Fiscal").first()).toBeVisible();

  const auth = api();
  const response = await request.get(`${auth.baseURL}/api/studio/agents?scope=workspace`, {
    headers: auth.headers,
  });
  expect(response.ok()).toBeTruthy();
  const agents = (await response.json()) as Array<{
    id: string;
    name?: string;
    avatar?: string;
    color?: string;
    path: string;
  }>;
  const agent = agents.find((item) => item.id === "revisor-fiscal");
  expect(agent).toBeTruthy();
  expect(agent?.name).toBe("Revisor Fiscal");
  expect(agent?.avatar).toBe("🧾");
  expect(agent?.color).toMatch(/^#[0-9a-f]{6}$/);

  // External edit must reach the UI through the watcher/WS.
  const content = readFileSync(agent!.path, "utf8").replace(
    "Revisor Fiscal",
    "Revisor Fiscal Externo",
  );
  writeFileSync(agent!.path, content);
  await expect(page.getByText("Revisor Fiscal Externo").first()).toBeVisible({
    timeout: 5000,
  });
});

test("adds an existing workspace from another folder without touching it", async ({ page }) => {
  const existing = "/tmp/opendesk-e2e/existing-workspace";
  mkdirSync(existing, { recursive: true });
  writeFileSync(`${existing}/keep-me.txt`, "do not touch\n");

  await page.goto("/");
  await page.getByTitle("Novo workspace").click();
  await page.getByRole("button", { name: "Adicionar existente" }).click();
  await page
    .locator('input[placeholder="/home/rafael/Projetos/minha-empresa"]')
    .fill(existing);
  await page
    .locator('input[placeholder="Neuralab, Bernardes AI, Home Lab…"]')
    .fill("Existing Workspace");
  await page.getByRole("button", { name: "Adicionar workspace" }).click();

  await expect(page.locator("select").first()).toContainText("Existing Workspace");
  expect(existsSync(`${existing}/keep-me.txt`)).toBeTruthy();
});
