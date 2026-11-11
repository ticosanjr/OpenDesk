import type { APIRequestContext } from "@playwright/test";
import { expect } from "@playwright/test";
import { api, seedWorkspace, type SeededWorkspace } from "../helpers";

export interface CompanySeed {
  workspace: SeededWorkspace;
  agentIds: string[];
  taskId?: string;
  runId?: string;
  artifactIds?: string[];
  hitlToken?: string;
  proposalId?: string;
  permissionId?: string;
}

const AGENTS = [
  {
    id: "atendimento",
    name: "Atendimento",
    description: "Triagem de clientes e follow-ups.",
  },
  {
    id: "pesquisa",
    name: "Pesquisa",
    description: "Pesquisa de mercado e fontes.",
  },
  {
    id: "build",
    name: "Executor",
    description: "Produz entregáveis e relatórios.",
  },
  {
    id: "revisor",
    name: "Revisor",
    description: "Revisa qualidade e riscos.",
  },
];

async function putAgent(
  request: APIRequestContext,
  workspaceId: string,
  id: string,
  name: string,
  description: string,
) {
  const auth = api();
  const response = await request.put(
    `${auth.baseURL}/api/studio/agent?id=${id}&scope=workspace&workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        name,
        description,
        mode: "subagent",
        system: `Você é ${name}. ${description}`,
      },
    },
  );
  expect(response.ok()).toBeTruthy();
}

async function putSkill(
  request: APIRequestContext,
  workspaceId: string,
  id: string,
  name: string,
) {
  const auth = api();
  const response = await request.put(
    `${auth.baseURL}/api/studio/skill?id=${id}&scope=workspace&workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        name,
        description: `Skill ${name}`,
        body: `## Quando usar\n\n${name}.`,
        files: [],
      },
    },
  );
  expect(response.ok()).toBeTruthy();
}

async function putCommand(
  request: APIRequestContext,
  workspaceId: string,
  id: string,
  description: string,
) {
  const auth = api();
  const response = await request.put(
    `${auth.baseURL}/api/studio/command?id=${id}&scope=workspace&workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        description,
        agent: "build",
        template: `Execute: $ARGUMENTS`,
      },
    },
  );
  expect(response.ok()).toBeTruthy();
}

async function putRecipe(request: APIRequestContext, workspaceId: string) {
  const auth = api();
  const yaml = `id: proposta-comercial
name: Proposta comercial
enabled: true
triggers:
  - type: MANUAL
    config: {}
flow:
  - type: step
    id: diagnostico
    name: Diagnóstico
    mode: build
    prompt: Analise o briefing e produza um diagnóstico.
  - type: hitl
    id: aprovacao
    name: Aprovar proposta
    interaction_type: BINARY_GATE
    prompt_template: Aprovar o envio da proposta?
    timeout_minutes: 240
    on_timeout: PAUSE_AND_ALERT
`;
  const response = await request.put(
    `${auth.baseURL}/api/recipes/proposta-comercial?workspace_id=${workspaceId}`,
    { headers: auth.headers, data: { yaml } },
  );
  expect(response.ok()).toBeTruthy();
}

export async function seedCompany(
  request: APIRequestContext,
  prefix = "Aurora Consultoria",
): Promise<CompanySeed> {
  const workspace = await seedWorkspace(request, prefix);
  const agentIds = AGENTS.map((agent) => agent.id);
  for (const agent of AGENTS) {
    await putAgent(
      request,
      workspace.id,
      agent.id,
      agent.name,
      agent.description,
    );
  }
  await putSkill(request, workspace.id, "proposta-comercial", "Proposta comercial");
  await putSkill(request, workspace.id, "relatorio-executivo", "Relatório executivo");
  await putCommand(request, workspace.id, "proposta", "Gera uma proposta comercial");
  await putCommand(request, workspace.id, "fechamento", "Resume o fechamento do mês");
  await putRecipe(request, workspace.id);

  const auth = api();
  const task = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Diagnóstico do cliente Aurora",
        description:
          "Analise o briefing do cliente e prepare um diagnóstico com próximos passos.",
        execution_mode: "BUILD_DIRECT",
        assigned_agent: "pesquisa",
        tags: ["cliente", "diagnostico"],
      },
    },
  );
  expect(task.ok()).toBeTruthy();
  const taskBody = (await task.json()) as { id: string };
  return { workspace, agentIds, taskId: taskBody.id };
}

export interface SeedOperationalOptions {
  run?: boolean;
  failedRun?: boolean;
  artifacts?: boolean;
  hitl?: boolean;
  proposal?: boolean;
  permission?: boolean;
  taskStatus?: string;
}

export async function seedOperational(
  request: APIRequestContext,
  company: CompanySeed,
  options: SeedOperationalOptions = {},
): Promise<CompanySeed> {
  const auth = api();
  const body: Record<string, unknown> = {
    workspace_id: company.workspace.id,
    task: {
      title: "Entrega do diagnóstico",
      description: "Produza o relatório executivo do diagnóstico.",
      agent: "build",
      tags: ["entrega", "cliente"],
      status: options.taskStatus ?? "TODO",
    },
    run: {
      status: options.failedRun ? "FAILED" : "SUCCESS",
      agent: "build",
      model: "opencode-go/glm-5.3",
      duration_ms: 4200,
      tokens_input: 1200,
      tokens_output: 800,
      cost_usd: 0.042,
      summary: "Relatório executivo produzido com sucesso.",
      error: options.failedRun ? "timed out after 60s" : undefined,
      error_code: options.failedRun ? "timeout" : undefined,
      events: [
        {
          created: 1000,
          type: "session.execution.started",
          data: {},
        },
        {
          created: 1200,
          type: "session.reasoning.delta",
          data: { delta: "Vou analisar o briefing e montar o relatório." },
        },
        {
          created: 1400,
          type: "session.tool.input.started",
          data: { id: "tool-1", name: "read" },
        },
        {
          created: 1450,
          type: "session.tool.input.ended",
          data: { id: "tool-1", text: "{\"path\":\"input/brief.md\"}" },
        },
        {
          created: 1500,
          type: "session.tool.success",
          data: {
            id: "tool-1",
            content: [{ type: "text", text: "conteúdo do briefing" }],
          },
        },
        {
          created: 1600,
          type: "session.permission.requested",
          data: { action: "bash", resource: "npm test" },
        },
        {
          created: 1800,
          type: "session.text.delta",
          data: { delta: "Relatório executivo com três recomendações." },
        },
        {
          created: 2200,
          type: "session.execution.finished",
          data: {},
        },
      ],
    },
    artifacts: options.artifacts === false
      ? []
      : [
          {
            file_path: "relatorio-executivo.md",
            content: "# Relatório executivo\n\nRecomendações do diagnóstico.",
            action_type: "CREATED",
            agent: "build",
            reason: "Entrega do diagnóstico",
          },
        ],
  };
  if (options.hitl) {
    body.hitl = {
      checkpoint_id: "aprovacao",
      interaction_type: "BINARY_GATE",
      title: "Proposta comercial",
      question: "Aprovar o envio da proposta ao cliente?",
      options: [],
      on_timeout: "PAUSE_AND_ALERT",
    };
  }
  if (options.proposal) {
    body.proposal = {
      kind: "SKILL",
      action: "CREATE",
      title: "Skill aprendida: Follow-up comercial",
      description: "Proposta de skill gerada no teste E2E.",
      payload: {
        id: "follow-up-comercial",
        scope: "workspace",
        name: "Follow-up comercial",
        description: "Padroniza follow-ups.",
        body: "## Quando usar\n\nApós uma proposta enviada.",
        files: [],
      },
    };
  }
  if (options.permission) {
    body.permission = {
      action: "bash",
      resources: ["npm install"],
      session_id: "e2e-session",
    };
  }
  const response = await request.post(`${auth.baseURL}/api/test/seed`, {
    headers: auth.headers,
    data: body,
  });
  expect(response.ok()).toBeTruthy();
  const seeded = (await response.json()) as {
    task_id?: string;
    run_id?: string;
    artifact_ids?: string[];
    hitl_token?: string;
    proposal_id?: string;
    permission_id?: string;
  };
  return {
    ...company,
    taskId: seeded.task_id ?? company.taskId,
    runId: seeded.run_id,
    artifactIds: seeded.artifact_ids,
    hitlToken: seeded.hitl_token,
    proposalId: seeded.proposal_id,
    permissionId: seeded.permission_id,
  };
}
