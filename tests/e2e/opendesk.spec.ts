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
  const auth = api();
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
  await page.getByRole("button", { name: "Salvar", exact: true }).first().click();
  await expect(page.getByText("salvo", { exact: true })).toBeVisible();

  // Engine card: reports the resolved binary candidates even when headless
  // mode deliberately keeps the daemon offline.
  await expect(page.getByText("OpenCode Engine")).toBeVisible();
  await expect(page.getByText("Atualizações do OpenDesk")).toBeVisible();
  await expect(page.getByText("Sobre & diagnóstico")).toBeVisible();
  const about = await request.get(`${auth.baseURL}/api/about`, {
    headers: auth.headers,
  });
  expect(about.ok()).toBeTruthy();
  expect((await about.json()).app.version).toBe("0.8.1");
  const engineStatus = await request.get(`${auth.baseURL}/api/engine/status`, {
    headers: auth.headers,
  });
  expect(engineStatus.ok()).toBeTruthy();
  const engine = (await engineStatus.json()) as {
    daemon: { state: string; online: boolean };
    candidates: string[];
  };
  expect(engine.daemon.online).toBe(false);
  expect(engine.daemon.state).toBe("offline");
  expect(engine.candidates.length).toBeGreaterThan(0);

  // Studio: create an agent with name, avatar and color.
  await page.getByRole("link", { name: "OpenCode Studio" }).click();
  await expect(page.getByRole("heading", { name: "OpenCode Studio" })).toBeVisible();
  await page.getByTitle("Novo agente").click();
  await page
    .locator('input[placeholder="Auditor de Qualidade"]')
    .fill("Revisor Fiscal");
  // The slug is derived from the display name; no manual ID is required.
  await expect(page.locator('input[placeholder="auditor-de-qualidade"]')).toHaveValue(
    "revisor-fiscal",
  );
  await page.getByTitle("🧾").click();
  await page.getByTitle("#fbbf24").click();
  await page
    .locator('textarea[placeholder^="Você é o Auditor"]')
    .fill("Você é o revisor fiscal do workspace.");
  await page.getByRole("button", { name: "Salvar" }).click();
  await expect(page.getByText("Revisor Fiscal").first()).toBeVisible();

  type AgentShape = {
    id: string;
    name?: string;
    avatar?: string;
    color?: string;
    path: string;
  };
  let agent: AgentShape | undefined;
  await expect
    .poll(
      async () => {
        const response = await request.get(
          `${auth.baseURL}/api/studio/agents?scope=workspace`,
          { headers: auth.headers },
        );
        if (!response.ok()) return false;
        const agents = (await response.json()) as AgentShape[];
        agent = agents.find((item) => item.id === "revisor-fiscal");
        return Boolean(agent);
      },
      { timeout: 5000 },
    )
    .toBeTruthy();

  expect(agent?.name).toBe("Revisor Fiscal");
  expect(agent?.avatar).toBe("🧾");
  expect(agent?.color).toBe("#fbbf24");

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

test("creates a Kanban task and enqueues it with the queue paused", async ({
  page,
  request,
}) => {
  const auth = api();
  const pause = await request.post(`${auth.baseURL}/api/queue/pause?reason=e2e`, {
    headers: auth.headers,
  });
  expect(pause.ok()).toBeTruthy();

  const concurrency = await request.put(`${auth.baseURL}/api/queue/concurrency`, {
    headers: auth.headers,
    data: { concurrency: 3 },
  });
  expect(concurrency.ok()).toBeTruthy();
  expect((await concurrency.json()).concurrency).toBe(3);
  await request.put(`${auth.baseURL}/api/queue/concurrency`, {
    headers: auth.headers,
    data: { concurrency: 1 },
  });

  await page.goto("/#/kanban");
  await expect(page.getByRole("heading", { name: "Kanban & Fila" })).toBeVisible();

  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  await page
    .getByPlaceholder("Relatório executivo de vendas")
    .fill("Tarefa E2E do Kanban");
  await page
    .getByPlaceholder("Descreva o objetivo, as entradas e os critérios de aceite…")
    .fill("Criar um artefato de teste e registrar proveniência.");
  await page.getByRole("button", { name: "Criar tarefa" }).click();

  await expect(page.getByText("Tarefa E2E do Kanban").first()).toBeVisible();
  await page.getByRole("button", { name: "Executar" }).first().click();
  await expect(page.getByText("QUEUED").first()).toBeVisible();

  const tasks = await request.get(`${auth.baseURL}/api/tasks`, {
    headers: auth.headers,
  });
  expect(tasks.ok()).toBeTruthy();
  const taskList = (await tasks.json()) as Array<{
    title: string;
    column_stage: string;
    output_dir_path?: string;
  }>;
  const task = taskList.find((item) => item.title === "Tarefa E2E do Kanban");
  expect(task).toBeTruthy();
  expect(task?.column_stage).toBe("BUILD");
  expect(task?.output_dir_path).toContain("/output/");

  expect(task).toBeTruthy();
  const diagnostics = await request.get(
    `${auth.baseURL}/api/tasks/${task!.id}/diagnostics`,
    { headers: auth.headers },
  );
  expect(diagnostics.ok()).toBeTruthy();
  const diagnosticsBody = await diagnostics.json();
  expect(diagnosticsBody.task.id).toBe(task!.id);
  expect(Array.isArray(diagnosticsBody.runs)).toBe(true);

  // DONE is a workflow action that does not require the engine.
  const done = await request.post(
    `${auth.baseURL}/api/tasks/${task!.id}/transition`,
    { headers: auth.headers, data: { column_stage: "DONE" } },
  );
  expect(done.ok()).toBeTruthy();
  expect((await done.json()).task.column_stage).toBe("DONE");

  await page.reload();
  await expect(page.getByText("Tarefa E2E do Kanban").first()).toBeVisible();
  await page.getByText("Tarefa E2E do Kanban").first().click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByRole("button", { name: "Editar" }).click();
  await expect(page.getByText("Editar tarefa")).toBeVisible();
  await page.getByLabel("Agente").fill("explore");
  await page.getByRole("button", { name: "Salvar", exact: true }).click();
  await expect(page.getByText("Editar tarefa")).not.toBeVisible();
  const tasksAfterEdit = await request.get(`${auth.baseURL}/api/tasks`, {
    headers: auth.headers,
  });
  const edited = (await tasksAfterEdit.json()) as Array<{
    id: string;
    assigned_agent?: string | null;
  }>;
  expect(edited.find((item) => item.id === task!.id)?.assigned_agent).toBe(
    "explore",
  );
});

test("creates a cron routine, runs it and saves SMTP/Google settings", async ({
  page,
  request,
}) => {
  const auth = api();
  await request.post(`${auth.baseURL}/api/queue/pause?reason=e2e-routine`, {
    headers: auth.headers,
  });

  await page.goto("/#/routines");
  await expect(page.getByRole("heading", { name: "Rotinas (Cron)" })).toBeVisible();
  await page.getByRole("main").getByRole("button", { name: "Nova rotina" }).click();
  await page.getByLabel("Nome").fill("Rotina E2E");
  await page.getByLabel("Comando ou prompt").fill("Produza um relatório de teste.");
  await page.getByLabel("Destinatários de e-mail").fill("e2e@example.com");
  await page.getByRole("dialog").getByRole("button", { name: "Criar rotina" }).click();
  await expect(page.getByText("Rotina E2E").first()).toBeVisible();
  await page.getByRole("button", { name: "Executar agora" }).first().click();
  await expect(page.getByText("schedule").first()).toBeVisible();

  await page.goto("/#/integrations");
  await expect(page.getByRole("heading", { name: "Integrações" })).toBeVisible();
  await expect(page.getByText("Signal (celular)")).toBeVisible();
  await page.getByLabel("Host").fill("smtp.example.com");
  await page.getByLabel("Remetente").fill("OpenDesk <e2e@example.com>");
  await page.getByLabel("Senha / app password").fill("e2e-secret");
  await page.getByRole("button", { name: "Salvar" }).first().click();
  await expect(page.getByText("configurado").first()).toBeVisible();

  await page.getByLabel("Client ID").fill("e2e.apps.googleusercontent.com");
  await page.getByLabel("Client secret").fill("e2e-google-secret");
  await page.getByRole("button", { name: "Salvar credenciais" }).click();
  await expect(page.getByText("Credenciais Google salvas.")).toBeVisible();
});


test("creates a chat thread and shows it in the UI", async ({ page, request }) => {
  const auth = api();
  const workspaces = await request.get(`${auth.baseURL}/api/workspaces`, {
    headers: auth.headers,
  });
  const list = (await workspaces.json()) as Array<{ id: string }>;
  const workspaceId = list[0]?.id;
  expect(workspaceId).toBeTruthy();

  const created = await request.post(`${auth.baseURL}/api/chat/threads`, {
    headers: auth.headers,
    data: {
      workspace_id: workspaceId,
      title: "Conversa E2E",
      agent: "build",
      source: "desktop",
    },
  });
  expect(created.ok()).toBeTruthy();
  const thread = (await created.json()) as { id: string };

  const detail = await request.get(
    `${auth.baseURL}/api/chat/threads/${thread.id}`,
    { headers: auth.headers },
  );
  expect(detail.ok()).toBeTruthy();
  expect((await detail.json()).messages).toEqual([]);

  await page.goto("/#/chat");
  await expect(page.getByRole("heading", { name: "Chat & Criador" })).toBeVisible();
  await expect(page.getByText("Conversa E2E").first()).toBeVisible();
  await expect(page.getByRole("heading", { name: "Conversas" })).toBeVisible();

  const removed = await request.delete(
    `${auth.baseURL}/api/chat/threads/${thread.id}`,
    { headers: auth.headers },
  );
  expect(removed.ok()).toBeTruthy();
});
