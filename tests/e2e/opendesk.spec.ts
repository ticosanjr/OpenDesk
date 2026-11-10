import { expect, test } from "@playwright/test";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { api } from "./helpers";

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
    .getByPlaceholder("~/Documents/OpenDesk Workareas")
    .fill("/tmp/opendesk-e2e/workspaces");
  await page.getByRole("button", { name: "Criar workspace" }).click();

  await expect(page.getByRole("button", { name: "Workspace ativo" })).toContainText("E2E Workspace");

  // Settings: change the default workspaces folder.
  await page.getByRole("link", { name: "Configurações" }).click();
  await expect(page.getByRole("heading", { name: "Configurações" })).toBeVisible();
  await page
    .getByPlaceholder("~/Documents/OpenDesk Workareas")
    .fill("/tmp/opendesk-e2e/default-root");
  await page.getByRole("button", { name: "Salvar", exact: true }).first().click();
  await expect(page.getByText("salvo", { exact: true })).toBeVisible();

  // Only the active tab is rendered.
  await expect(page.getByText("OpenCode Engine")).toHaveCount(0);
  await page.getByRole("tab", { name: "Engine" }).click();
  await expect(page.getByText("OpenCode Engine")).toBeVisible();
  await page.getByRole("tab", { name: "Atualizações" }).click();
  await expect(page.getByText("Atualizações do OpenDesk")).toBeVisible();
  await page.getByRole("tab", { name: "Sobre" }).click();
  await expect(page.getByText("Sobre & diagnóstico")).toBeVisible();
  const about = await request.get(`${auth.baseURL}/api/about`, {
    headers: auth.headers,
  });
  expect(about.ok()).toBeTruthy();
  const packageVersion = (
    JSON.parse(
      readFileSync(new URL("../../package.json", import.meta.url), "utf8"),
    ) as { version: string }
  ).version;
  expect((await about.json()).app.version).toBe(packageVersion);
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
  await page.getByRole("button", { name: /^Workspace ·/ }).click();
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
  await page.getByRole("button", { name: "Existente" }).click();
  await page
    .locator('input[placeholder="/home/rafael/Projetos/minha-empresa"]')
    .fill(existing);
  await page
    .locator('input[placeholder="Neuralab, Bernardes AI, Home Lab…"]')
    .fill("Existing Workspace");
  await page.getByRole("button", { name: "Adicionar workspace" }).click();

  await expect(page.getByRole("button", { name: "Workspace ativo" })).toContainText("Existing Workspace");
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

  const tasks = await request.get(`${auth.baseURL}/api/tasks`, {
    headers: auth.headers,
  });
  expect(tasks.ok()).toBeTruthy();
  const taskList = (await tasks.json()) as Array<{
    id: string;
    title: string;
    status: string;
  }>;
  const task = taskList.find((item) => item.title === "Tarefa E2E do Kanban");
  expect(task).toBeTruthy();
  expect(task?.status).toBe("TODO");

  const diagnostics = await request.get(
    `${auth.baseURL}/api/tasks/${task!.id}/diagnostics`,
    { headers: auth.headers },
  );
  expect(diagnostics.ok()).toBeTruthy();
  const diagnosticsBody = await diagnostics.json();
  expect(diagnosticsBody.task.id).toBe(task!.id);
  expect(Array.isArray(diagnosticsBody.runs)).toBe(true);

  for (const status of ["TODO", "IN_PROGRESS", "IN_REVIEW", "DONE", "BLOCKED", "CANCELLED", "TODO"]) {
    const moved = await request.post(
      `${auth.baseURL}/api/tasks/${task!.id}/transition`,
      { headers: auth.headers, data: { status } },
    );
    expect(moved.ok()).toBeTruthy();
    expect((await moved.json()).task.status).toBe(status);
  }

  await page.reload();
  await expect(page.getByText("Tarefa E2E do Kanban").first()).toBeVisible();
  await page.getByRole("button", { name: "Abrir tarefa Tarefa E2E do Kanban" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByRole("button", { name: "Editar" }).click();
  await expect(page.getByText("Editar tarefa")).toBeVisible();
  await page.getByRole("button", { name: "Mais opções" }).click();
  await page.getByLabel("Agente", { exact: true }).fill("explore");
  await page.getByRole("option", { name: /@explore/ }).click();
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
  await page.getByRole("button", { name: "Mais opções" }).click();
  await page.getByRole("button", { name: "Modelo específico" }).click();
  const modelInput = page.getByPlaceholder(/digite para filtrar/);
  await modelInput.fill("opencode-go/glm-5.3#high");
  await page.keyboard.press("Tab");
  await page.getByRole("button", { name: "Salvar", exact: true }).click();
  await expect(page.getByText("Editar tarefa")).not.toBeVisible();
  await expect(page.getByText("Tarefa atualizada").first()).toBeVisible();

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

test("creates and edits a recurrence routine with calendar, agent search and notifications", async ({
  page,
  request,
}) => {
  const auth = api();
  await request.post(`${auth.baseURL}/api/queue/pause?reason=e2e-routine`, {
    headers: auth.headers,
  });

  // Make this test self-contained when it runs alone.
  const existing = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
    })
  ).json()) as Array<{ id: string }>;
  if (existing.length === 0) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: {
        name: "Rotinas E2E",
        root_path: "/tmp/opendesk-e2e/workspaces-routines",
      },
    });
    expect(created.ok()).toBeTruthy();
  }

  // Recurrence preview is computed server-side before any schedule is stored.
  const preview = await request.post(`${auth.baseURL}/api/schedules/preview`, {
    headers: auth.headers,
    data: {
      timezone: "America/Sao_Paulo",
      recurrence: {
        frequency: "weekdays",
        interval: 1,
        time: "07:45",
        weekdays: [],
      },
    },
  });
  expect(preview.ok()).toBeTruthy();
  const previewBody = (await preview.json()) as {
    cron_expression: string;
    human: string;
    next_runs: string[];
  };
  expect(previewBody.cron_expression).toBe("45 7 * * 1-5");
  expect(previewBody.human).toContain("dias úteis");
  expect(previewBody.next_runs).toHaveLength(5);

  const invalid = await request.post(`${auth.baseURL}/api/schedules/preview`, {
    headers: auth.headers,
    data: {
      timezone: "America/Sao_Paulo",
      recurrence: {
        frequency: "daily",
        interval: 1,
        time: "25:00",
        weekdays: [],
      },
    },
  });
  expect(invalid.status()).toBe(400);

  await page.goto("/#/routines");
  await expect(page.getByRole("heading", { name: "Rotinas & Gatilhos" })).toBeVisible();
  await page.getByRole("main").getByRole("button", { name: "Nova rotina" }).click();

  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Nome da rotina").fill("Rotina E2E");
  await dialog.getByRole("button", { name: "Dias úteis" }).click();
  await expect(dialog.getByText("dias úteis às", { exact: false })).toBeVisible();

  // Timezone defaults to the local machine timezone and stays editable.
  const timezone = await dialog.getByLabel("Fuso horário").inputValue();
  expect(timezone.length).toBeGreaterThan(0);

  // Agent picker is optional and filters as the user types.
  const agentInput = dialog.getByPlaceholder("automático (router)");
  await agentInput.fill("global-agent");
  await expect(
    dialog.getByRole("button", { name: /@global-agent/ }),
  ).toBeVisible();
  await agentInput.press("Enter");
  await expect(agentInput).toHaveValue("global-agent");

  await dialog.getByLabel("E-mail", { exact: true }).fill("e2e@example.com");
  await dialog.getByLabel("notificar por Signal").check();
  await dialog
    .getByLabel("notificar somente se houver novidade")
    .check();
  await dialog
    .getByLabel("Comando ou prompt")
    .fill("Produza um relatório de teste. /global-command");
  await expect(dialog.getByText(/Nenhum comando\/skill encontrado/)).toBeVisible();
  await dialog.getByRole("button", { name: "Criar rotina" }).click();
  await expect(page.getByText("Rotina E2E").first()).toBeVisible();
  await expect(page.getByText("dias úteis às", { exact: false }).first()).toBeVisible();
  await expect(page.getByText("só novidades").first()).toBeVisible();

  type ScheduleShape = {
    id: string;
    name: string;
    cron_expression: string;
    timezone: string;
    agent_name?: string | null;
    notify_signal: boolean;
    notify_only_on_change: boolean;
    recurrence_json?: string | null;
    one_time: boolean;
  };
  let routine: ScheduleShape | undefined;
  await expect
    .poll(
      async () => {
        const response = await request.get(`${auth.baseURL}/api/schedules`, {
          headers: auth.headers,
        });
        if (!response.ok()) return false;
        const list = (await response.json()) as ScheduleShape[];
        routine = list.find((item) => item.name === "Rotina E2E");
        return Boolean(routine);
      },
      { timeout: 5_000 },
    )
    .toBeTruthy();
  expect(routine?.cron_expression).toBe("0 8 * * 1-5");
  expect(routine?.timezone).toBe(timezone);
  expect(routine?.agent_name).toBe("global-agent");
  expect(routine?.notify_signal).toBe(true);
  expect(routine?.notify_only_on_change).toBe(true);
  expect(routine?.one_time).toBe(false);
  expect(routine?.recurrence_json).toContain('"frequency":"weekdays"');

  // Editing reuses the same calendar UI and persists the new name.
  await page.getByRole("button", { name: "Editar" }).first().click();
  await expect(dialog.getByLabel("Nome da rotina")).toHaveValue("Rotina E2E");
  await dialog.getByLabel("Nome da rotina").fill("Rotina E2E editada");
  await dialog.getByRole("button", { name: "Salvar rotina" }).click();
  await expect(page.getByText("Rotina E2E editada").first()).toBeVisible();

  await page.getByRole("button", { name: "Executar agora" }).first().click();
  await expect(page.getByText("schedule").first()).toBeVisible();

  await page.goto("/#/integrations");
  await expect(page.getByRole("heading", { name: "Integrações" })).toBeVisible();
  await page.getByLabel("Host").fill("smtp.example.com");
  await page.getByLabel("Remetente").fill("OpenDesk <e2e@example.com>");
  await page.getByLabel("Senha / app password").fill("e2e-secret");
  await page.getByRole("button", { name: "Salvar" }).first().click();
  await expect(page.getByText("configurado").first()).toBeVisible();

  await page.getByRole("tab", { name: "Google" }).click();
  await page.getByLabel("Client ID").fill("e2e.apps.googleusercontent.com");
  await page.getByLabel("Client secret").fill("e2e-google-secret");
  await page.getByRole("button", { name: "Salvar credenciais" }).click();
  await expect(page.getByText("Credenciais Google salvas.")).toBeVisible();

  await page.getByRole("tab", { name: "Signal" }).click();
  await expect(page.getByText("Signal (celular)")).toBeVisible();
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
  await expect(page.getByText("rea", { exact: true }).first()).toBeVisible();
  await page.getByRole("button", { name: "Global" }).click();
  await expect(page.getByText("Global Agent").first()).toBeVisible();
  await page.getByRole("button", { name: "Skills" }).click();
  await expect(page.getByText("global-skill").first()).toBeVisible();
  await page.getByRole("button", { name: "Comandos" }).click();
  await expect(page.getByText("global-command").first()).toBeVisible();
  await page.getByRole("button", { name: "MCP" }).click();
  await expect(page.getByText("rea", { exact: true }).first()).toBeVisible();
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

test("kanban move control persists an adjacent column", async ({ page, request }) => {
  const auth = api();
  const existingWorkspaces = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  if (existingWorkspaces.length === 0) {
    const createdWorkspace = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: {
        name: "Drag Workspace",
        root_path: "/tmp/opendesk-e2e/workspaces-drag",
      },
    });
    expect(createdWorkspace.ok()).toBeTruthy();
  }
  const workspaces = await request.get(`${auth.baseURL}/api/workspaces`, {
    headers: auth.headers,
  });
  const workspaceId = ((await workspaces.json()) as Array<{ id: string }>)[0]?.id;
  expect(workspaceId).toBeTruthy();

  const created = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        title: "Drag E2E",
        description: "Teste de drag",
        execution_mode: "AUTO_DSH",
        priority: 2,
        tags: [],
        effort_level: "medium",
      },
    },
  );
  expect(created.ok()).toBeTruthy();
  const createdTask = (await created.json()) as { id: string; status: string };
  expect(createdTask.status).toBe("TODO");
  const taskId = createdTask.id;

  await page.goto("/#/kanban");
  await expect(page.getByText("Backlog", { exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Abrir tarefa Drag E2E" })).toBeVisible();
  await page
    .getByRole("button", { name: "Abrir tarefa Drag E2E" })
    .dragTo(page.locator('[data-column="TODO"]'));
  await expect(page.locator('[data-column="TODO"]').getByText("Drag E2E")).toBeVisible();

  // The column change is persisted through an async API call; poll briefly.
  await expect
    .poll(
      async () => {
        const tasks = await request.get(`${auth.baseURL}/api/tasks`, {
          headers: auth.headers,
        });
        if (!tasks.ok()) return null;
        const list = (await tasks.json()) as Array<{
          id: string;
          status: string;
        }>;
        return list.find((item) => item.id === taskId)?.status ?? null;
      },
      { timeout: 5_000 },
    )
    .toBe("TODO");
});

test("studio backup lists and restores an item", async ({ request }) => {
  const auth = api();
  await request.put(`${auth.baseURL}/api/studio/agent?id=backup-e2e&scope=global`, {
    headers: auth.headers,
    data: { name: "V1", mode: "subagent", system: "versão 1" },
  });
  await request.put(`${auth.baseURL}/api/studio/agent?id=backup-e2e&scope=global`, {
    headers: auth.headers,
    data: { name: "V2", mode: "subagent", system: "versão 2" },
  });
  const backups = await request.get(
    `${auth.baseURL}/api/studio/agent/backup-e2e/backups?scope=global`,
    { headers: auth.headers },
  );
  expect(backups.ok()).toBeTruthy();
  const entries = (await backups.json()).backups as Array<{ id: string }>;
  expect(entries.length).toBeGreaterThan(0);

  const restored = await request.post(
    `${auth.baseURL}/api/studio/agent/backup-e2e/restore?scope=global`,
    { headers: auth.headers, data: { backup_id: entries[0].id } },
  );
  expect(restored.ok()).toBeTruthy();
  const agent = await request.get(
    `${auth.baseURL}/api/studio/agent?id=backup-e2e&scope=global`,
    { headers: auth.headers },
  );
  expect((await agent.json()).name).toBe("V1");

  await request.delete(`${auth.baseURL}/api/studio/agent/backup-e2e?scope=global`, {
    headers: auth.headers,
  });
});

test("files explorer lists and previews workspace files", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspaces = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string; root_path: string }>;
  const workspace = workspaces[0];
  expect(workspace).toBeTruthy();
  const inputDir = `${workspace.root_path}/input`;
  mkdirSync(inputDir, { recursive: true });
  writeFileSync(`${inputDir}/e2e-brief.md`, "# Brief E2E\n\nConteúdo do explorador.\n");

  const tree = await request.get(
    `${auth.baseURL}/api/files/tree?workspace_id=${encodeURIComponent(workspace.id)}&path=input`,
    { headers: auth.headers },
  );
  expect(tree.ok()).toBeTruthy();
  const entries = (await tree.json()) as Array<{ name: string; path: string }>;
  expect(entries.some((entry) => entry.name === "e2e-brief.md")).toBeTruthy();

  await page.goto("/#/files");
  await expect(page.getByRole("heading", { name: "Arquivos" })).toBeVisible();
  await page.getByRole("button", { name: /input/ }).first().click();
  await page.getByRole("button", { name: /e2e-brief\.md/ }).click();
  await expect(page.getByText("Conteúdo do explorador.")).toBeVisible();

  // Deep link used by run tool paths and file references.
  await page.goto("/#/files?path=input/e2e-brief.md");
  await expect(page.getByText("Conteúdo do explorador.")).toBeVisible();
});

test("edits and removes a workspace without touching its folder", async ({
  page,
  request,
}) => {
  const auth = api();
  const original = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;

  const created = await request.post(`${auth.baseURL}/api/workspaces`, {
    headers: auth.headers,
    data: {
      name: "Workspace Editável",
      root_path: "/tmp/opendesk-e2e/editable-workspace",
    },
  });
  expect(created.ok()).toBeTruthy();
  const workspace = (await created.json()) as { id: string; root_path: string };

  await page.goto("/#/settings?section=workspaces");
  await expect(page.getByText("Workspace Editável").first()).toBeVisible();
  const row = page
    .locator("p")
    .filter({ hasText: "Workspace Editável" })
    .first()
    .locator("xpath=ancestor::div[contains(@class,'rounded-lg')][1]");
  await row.getByRole("button", { name: "Editar" }).click();
  await page.getByLabel("Nome").fill("Workspace Renomeado");
  await page.getByLabel("Fuso horário").selectOption("America/Sao_Paulo");
  await page.getByRole("button", { name: "Salvar workspace" }).click();
  await expect(page.getByText("Workspace atualizado").first()).toBeVisible();

  const renamed = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string; name: string; timezone: string }>;
  expect(renamed.find((item) => item.id === workspace.id)?.name).toBe(
    "Workspace Renomeado",
  );
  expect(renamed.find((item) => item.id === workspace.id)?.timezone).toBe(
    "America/Sao_Paulo",
  );

  const updatedRow = page
    .locator("p")
    .filter({ hasText: "Workspace Renomeado" })
    .first()
    .locator("xpath=ancestor::div[contains(@class,'rounded-lg')][1]");
  await updatedRow.getByRole("button", { name: "Remover" }).click();
  await page.getByRole("button", { name: "Remover registro" }).click();
  await expect(page.getByText("Workspace removido").first()).toBeVisible();

  const remaining = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  expect(remaining.some((item) => item.id === workspace.id)).toBe(false);
  expect(existsSync(workspace.root_path)).toBe(true);

  if (original[0]) {
    await request.post(
      `${auth.baseURL}/api/workspaces/${original[0].id}/activate`,
      { headers: auth.headers },
    );
  }
});

test("runs route is reachable from the sidebar", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Execuções" }).click();
  await expect(page.getByRole("heading", { name: "Execuções" })).toBeVisible();
});

test("global search finds tasks and opens them", async ({ page, request }) => {
  const auth = api();
  const existing = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  let workspaceId = existing[0]?.id;
  if (!workspaceId) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: { name: "Search Workspace", root_path: "/tmp/opendesk-e2e/workspaces-search" },
    });
    workspaceId = ((await created.json()) as { id: string }).id;
  }
  const task = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        title: "Busca Global E2E",
        description: "Documento localizável pela busca global.",
        execution_mode: "AUTO_DSH",
        priority: 2,
        tags: [],
        effort_level: "medium",
      },
    },
  );
  expect(task.ok()).toBeTruthy();

  const apiResults = await request.get(
    `${auth.baseURL}/api/search?workspace_id=${workspaceId}&q=Busca%20Global`,
    { headers: auth.headers },
  );
  expect(apiResults.ok()).toBeTruthy();
  const hits = (await apiResults.json()) as Array<{ kind: string; title: string }>;
  expect(hits.some((hit) => hit.kind === "task" && hit.title.includes("Busca Global"))).toBe(true);

  await page.goto("/#/search");
  await page.getByPlaceholder("Digite pelo menos 2 caracteres…").fill("Busca Global E2E");
  await expect(page.getByText("Busca Global E2E").first()).toBeVisible();
  await page.locator('a[href*="/kanban?task="]').first().click();
  await expect(page).toHaveURL(/kanban/);
  await expect(page.getByRole("dialog")).toBeVisible();
});

test("task can be cancelled from the task panel at any time", async ({ page, request }) => {
  const auth = api();
  const existing = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  let workspaceId = existing[0]?.id;
  if (!workspaceId) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: { name: "Cancel Workspace", root_path: "/tmp/opendesk-e2e/workspaces-cancel" },
    });
    workspaceId = ((await created.json()) as { id: string }).id;
  }
  const createdTask = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        title: "Cancelar E2E",
        description: "Tarefa para testar cancelamento.",
        execution_mode: "AUTO_DSH",
        priority: 2,
        tags: [],
        effort_level: "medium",
      },
    },
  );
  expect(createdTask.ok()).toBeTruthy();
  const task = (await createdTask.json()) as { id: string };
  await request.post(`${auth.baseURL}/api/tasks/${task.id}/transition`, {
    headers: auth.headers,
    data: { status: "IN_PROGRESS" },
  });

  await page.goto("/#/kanban");
  await page.getByRole("button", { name: "Abrir tarefa Cancelar E2E" }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Cancelar", exact: true })
    .click();
  await expect(page.getByText("Tarefa movida para Cancelled").first()).toBeVisible();

  const tasks = await request.get(`${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`, {
    headers: auth.headers,
  });
  const list = (await tasks.json()) as Array<{ id: string; status: string }>;
  expect(list.find((item) => item.id === task.id)?.status).toBe("CANCELLED");
});

test("notification preferences and kanban list view are available", async ({
  page,
  request,
}) => {
  const auth = api();
  const existing = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  let workspaceId = existing[0]?.id;
  if (!workspaceId) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: { name: "Prefs Workspace", root_path: "/tmp/opendesk-e2e/workspaces-prefs" },
    });
    workspaceId = ((await created.json()) as { id: string }).id;
  }
  await request.post(`${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`, {
    headers: auth.headers,
    data: {
      title: "Tarefa Lista E2E",
      description: "Visível na tabela do Kanban.",
      tags: [],
    },
  });

  await page.goto("/#/settings");
  await expect(page.getByText("Pasta padrão de workspaces")).toBeVisible();
  await expect(page.getByText("OpenCode Engine")).toHaveCount(0);
  await page.getByRole("tab", { name: "Engine" }).click();
  await expect(page.getByText("OpenCode Engine")).toBeVisible();
  await expect(page.getByText("Pasta padrão de workspaces")).toHaveCount(0);
  await page.getByRole("tab", { name: "Notificações" }).click();
  await expect(page.getByText("Notificações e fechamento")).toBeVisible();
  await expect(page.getByText("Fechar janela mantendo na bandeja")).toBeVisible();

  await page.goto("/#/integrations");
  await expect(page.getByText("Host", { exact: true })).toBeVisible();
  await expect(page.getByText("OAuth2 loopback + PKCE")).toHaveCount(0);
  await page.getByRole("tab", { name: "Google" }).click();
  await expect(page.getByText("OAuth2 loopback + PKCE")).toBeVisible();
  await expect(page.getByText("Host", { exact: true })).toHaveCount(0);

  await page.goto("/#/kanban");
  await page.getByTitle("Lista").click();
  await expect(page.getByText("Último run")).toBeVisible();
  await expect(page.getByText("Tarefa Lista E2E")).toBeVisible();
  await page.getByTitle("Fila").click();
  await expect(page.getByText("Controlador da fila")).toBeVisible();
  // The view switch must stay visible inside the queue view.
  await page.getByTitle("Board").click();
  await expect(page.getByPlaceholder(/Buscar por título/)).toBeVisible();

  const saved = await request.put(`${auth.baseURL}/api/settings/close_to_tray`, {
    headers: auth.headers,
    data: true,
  });
  expect(saved.ok()).toBeTruthy();
  const value = await request.get(
    `${auth.baseURL}/api/settings/close_to_tray`,
    { headers: auth.headers },
  );
  expect(await value.json()).toBe(true);
});
