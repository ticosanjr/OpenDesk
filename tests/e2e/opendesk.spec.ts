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
  expect((await about.json()).app.version).toBe("0.11.0");
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
  await page.getByRole("button", { name: /Workspace/ }).click();
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

  // Moving a card must persist even when the engine is offline; the dispatch
  // failure is reported separately and the column stays where the user put it.
  const planTransition = await request.post(
    `${auth.baseURL}/api/tasks/${task!.id}/transition`,
    { headers: auth.headers, data: { column_stage: "PLAN" } },
  );
  expect(planTransition.ok()).toBeTruthy();
  const planBody = await planTransition.json();
  expect(planBody.task.column_stage).toBe("PLAN");
  expect(planBody.error).toBeTruthy();
  expect(planBody.task.phase_status).toBe("dispatch_failed");

  const backlog = await request.post(
    `${auth.baseURL}/api/tasks/${task!.id}/transition`,
    { headers: auth.headers, data: { column_stage: "BACKLOG" } },
  );
  expect(backlog.ok()).toBeTruthy();
  expect((await backlog.json()).task.column_stage).toBe("BACKLOG");

  await page.reload();
  await expect(page.getByText("Tarefa E2E do Kanban").first()).toBeVisible();
  await page.getByRole("button", { name: "Abrir tarefa Tarefa E2E do Kanban" }).click();
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

  // UI edit of the model, including the type-to-filter combobox.
  await page.getByRole("button", { name: "Abrir tarefa Tarefa E2E do Kanban" }).click();
  await page.getByRole("button", { name: "Editar" }).click();
  const modelInput = page.getByPlaceholder(/digite para filtrar/);
  await modelInput.fill("opencode-go/glm-5.3#high");
  await page.getByRole("button", { name: "opencode-go/glm-5.3#high", exact: true }).click();
  await page.getByRole("button", { name: "Salvar", exact: true }).click();
  await expect(page.getByText("Editar tarefa")).not.toBeVisible();
  await expect(page.getByText("Tarefa salva")).toBeVisible();

  const afterModel = await request.get(`${auth.baseURL}/api/tasks`, {
    headers: auth.headers,
  });
  const modelTask = (await afterModel.json()) as Array<{
    id: string;
    preferred_model?: string | null;
  }>;
  expect(modelTask.find((item) => item.id === task!.id)?.preferred_model).toBe(
    "opencode-go/glm-5.3#high",
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


test("studio copies, toggles and installs across scopes", async ({ page, request }) => {
  const auth = api();
  const workspaces = await request.get(`${auth.baseURL}/api/workspaces`, {
    headers: auth.headers,
  });
  const workspaceId = ((await workspaces.json()) as Array<{ id: string }>)[0]?.id;
  expect(workspaceId).toBeTruthy();

  const globalAgent = await request.put(
    `${auth.baseURL}/api/studio/agent?id=studio-e2e&scope=global`,
    {
      headers: auth.headers,
      data: {
        name: "Studio E2E",
        avatar: "🧪",
        description: "Agente de teste",
        mode: "subagent",
        system: "Teste.",
      },
    },
  );
  expect(globalAgent.ok()).toBeTruthy();

  const copied = await request.post(
    `${auth.baseURL}/api/studio/agent/studio-e2e/copy?scope=global`,
    {
      headers: auth.headers,
      data: {
        target_scope: "workspace",
        target_workspace_id: workspaceId,
        new_id: "studio-e2e-ws",
      },
    },
  );
  expect(copied.ok()).toBeTruthy();

  const toggled = await request.post(
    `${auth.baseURL}/api/studio/agent/studio-e2e-ws/toggle?scope=workspace&workspace_id=${workspaceId}`,
    { headers: auth.headers, data: { enabled: false } },
  );
  expect(toggled.ok()).toBeTruthy();

  const effective = await request.get(
    `${auth.baseURL}/api/studio/effective?workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  expect(effective.ok()).toBeTruthy();
  const items = (await effective.json()).items as Array<{
    id: string;
    source_scope: string;
    disabled: boolean;
    kind: string;
  }>;
  const workspaceAgent = items.find(
    (item) => item.id === "studio-e2e-ws" && item.kind === "agent",
  );
  expect(workspaceAgent?.source_scope).toBe("workspace");
  expect(workspaceAgent?.disabled).toBe(true);

  const preview = await request.post(
    `${auth.baseURL}/api/studio/install/preview`,
    {
      headers: auth.headers,
      data: {
        source_type: "npm",
        value: "@modelcontextprotocol/server-memory",
        name: "memory-e2e",
      },
    },
  );
  expect(preview.ok()).toBeTruthy();
  const previewBody = await preview.json();
  const applied = await request.post(
    `${auth.baseURL}/api/studio/install/apply`,
    {
      headers: auth.headers,
      data: {
        items: previewBody.items,
        target_scope: "workspace",
        target_workspace_id: workspaceId,
      },
    },
  );
  expect(applied.ok()).toBeTruthy();

  await page.goto("/#/studio");
  await expect(page.getByRole("heading", { name: "OpenCode Studio" })).toBeVisible();
  await page.getByRole("button", { name: "Efetivo" }).click();
  await expect(page.getByText("studio-e2e-ws").first()).toBeVisible();
  await expect(page.getByRole("button", { name: "Instalar" })).toBeVisible();

  await request.delete(
    `${auth.baseURL}/api/studio/agent/studio-e2e-ws?scope=workspace&workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  await request.delete(
    `${auth.baseURL}/api/studio/mcp/memory-e2e?scope=workspace&workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  await request.delete(`${auth.baseURL}/api/studio/agent/studio-e2e?scope=global`, {
    headers: auth.headers,
  });
});

test("studio reads global V1 MCP, skills, agents and commands", async ({ page, request }) => {
  const auth = api();
  const query = "scope=global";

  const agents = await request.get(`${auth.baseURL}/api/studio/agents?${query}`, {
    headers: auth.headers,
  });
  expect(agents.ok()).toBeTruthy();
  expect((await agents.json()).some((item: { id: string }) => item.id === "global-agent")).toBe(true);

  const skills = await request.get(`${auth.baseURL}/api/studio/skills?${query}`, {
    headers: auth.headers,
  });
  expect(skills.ok()).toBeTruthy();
  expect((await skills.json()).some((item: { id: string }) => item.id === "global-skill")).toBe(true);

  const commands = await request.get(`${auth.baseURL}/api/studio/commands?${query}`, {
    headers: auth.headers,
  });
  expect(commands.ok()).toBeTruthy();
  expect((await commands.json()).some((item: { id: string }) => item.id === "global-command")).toBe(true);

  const mcp = await request.get(`${auth.baseURL}/api/studio/mcp?${query}`, {
    headers: auth.headers,
  });
  expect(mcp.ok()).toBeTruthy();
  const mcpBody = await mcp.json();
  expect(mcpBody.servers.some((item: { name: string }) => item.name === "rea")).toBe(true);

  const toggled = await request.post(
    `${auth.baseURL}/api/studio/mcp/rea/toggle?${query}`,
    { headers: auth.headers, data: { enabled: false } },
  );
  expect(toggled.ok()).toBeTruthy();

  const configAfter = await request.get(
    `${auth.baseURL}/api/studio/config?${query}`,
    { headers: auth.headers },
  );
  const parsed = (await configAfter.json()).parsed;
  expect(parsed.mcp.rea.enabled).toBe(false);
  expect(parsed.mcp.servers?.rea).toBeUndefined();

  await request.post(`${auth.baseURL}/api/studio/mcp/rea/toggle?${query}`, {
    headers: auth.headers,
    data: { enabled: true },
  });

  await page.goto("/#/studio");
  await expect(page.getByRole("heading", { name: "OpenCode Studio" })).toBeVisible();
  // Effective is the default and must include the global fixture.
  await expect(page.getByText("global-skill").first()).toBeVisible();
  await expect(page.getByText("rea").first()).toBeVisible();
  await page.getByRole("button", { name: "Global" }).click();
  await expect(page.getByText("Global Agent").first()).toBeVisible();
  await page.getByRole("button", { name: "Skills" }).click();
  await expect(page.getByText("global-skill").first()).toBeVisible();
  await page.getByRole("button", { name: "Comandos" }).click();
  await expect(page.getByText("global-command").first()).toBeVisible();
  await page.getByRole("button", { name: "MCP" }).click();
  await expect(page.getByText("rea").first()).toBeVisible();
});

test("studio imports local frontmatter and skill assets", async ({ request }) => {
  const auth = api();
  const workspaces = await request.get(`${auth.baseURL}/api/workspaces`, {
    headers: auth.headers,
  });
  const workspaceId = ((await workspaces.json()) as Array<{ id: string }>)[0]?.id;
  expect(workspaceId).toBeTruthy();

  const root = "/tmp/opendesk-e2e/import-fixture";
  mkdirSync(`${root}/agents`, { recursive: true });
  mkdirSync(`${root}/skills/revisor`, { recursive: true });
  writeFileSync(
    `${root}/agents/revisor.md`,
    `---
name: Revisor Importado
description: Revisa tudo
mode: subagent
model: deepseek/deepseek-flash
avatar: "🧾"
permissions:
  - action: edit
    resource: "*"
    effect: deny
---
Você revisa o workspace.
`,
  );
  writeFileSync(
    `${root}/skills/revisor/SKILL.md`,
    `---
name: Revisor Skill
description: Skill importada
---
Use a skill importada.
`,
  );
  writeFileSync(`${root}/skills/revisor/extra.md`, "conteúdo extra");

  const preview = await request.post(`${auth.baseURL}/api/studio/install/preview`, {
    headers: auth.headers,
    data: { source_type: "local", value: root },
  });
  expect(preview.ok()).toBeTruthy();
  const previewBody = await preview.json();
  const agentItem = previewBody.items.find(
    (item: { kind: string; id: string }) => item.kind === "agent" && item.id === "revisor",
  );
  const skillItem = previewBody.items.find(
    (item: { kind: string; id: string }) => item.kind === "skill" && item.id === "revisor",
  );
  expect(agentItem).toBeTruthy();
  expect(skillItem).toBeTruthy();
  expect(skillItem.files.some((file: { path: string }) => file.path === "extra.md")).toBe(true);

  const applied = await request.post(`${auth.baseURL}/api/studio/install/apply`, {
    headers: auth.headers,
    data: {
      items: previewBody.items,
      target_scope: "workspace",
      target_workspace_id: workspaceId,
      overwrite: true,
    },
  });
  expect(applied.ok()).toBeTruthy();

  const agent = await request.get(
    `${auth.baseURL}/api/studio/agent?id=revisor&scope=workspace&workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  expect(agent.ok()).toBeTruthy();
  const agentBody = await agent.json();
  expect(agentBody.model).toBe("deepseek/deepseek-flash");
  expect(agentBody.permissions).toHaveLength(1);
  expect(agentBody.system).toContain("Você revisa o workspace.");

  const skill = await request.get(
    `${auth.baseURL}/api/studio/skill?id=revisor&scope=workspace&workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  expect(skill.ok()).toBeTruthy();
  expect(
    (await skill.json()).files.some(
      (file: { path: string; content: string }) =>
        file.path === "extra.md" && file.content === "conteúdo extra",
    ),
  ).toBe(true);
});
