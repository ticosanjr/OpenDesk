import { expect, test } from "@playwright/test";
import { existsSync, readFileSync } from "node:fs";
import { api } from "./helpers";


test("agent configuration proposals require explicit approval", async ({ request }) => {
  const auth = api();
  const workspaces = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  let workspaceId = workspaces[0]?.id;
  if (!workspaceId) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: { name: "Proposals", root_path: "/tmp/opendesk-e2e/proposals" },
    });
    workspaceId = ((await created.json()) as { id: string }).id;
  }

  const workspace = (await (
    await request.get(`${auth.baseURL}/api/workspaces/${workspaceId}`, { headers: auth.headers })
  ).json()) as { root_path: string };
  const bridgeConfig = `${workspace.root_path}/opencode.jsonc`;
  expect(existsSync(bridgeConfig)).toBeTruthy();
  expect(readFileSync(bridgeConfig, "utf8")).toContain("opendesk-config");

  const created = await request.post(
    `${auth.baseURL}/api/proposals?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        kind: "AGENT",
        action: "CREATE",
        title: "Criar agente de propostas",
        description: "Teste de aprovação HITL",
        payload: {
          id: "agente-proposto",
          name: "Agente Proposto",
          description: "Criado após aprovação",
          mode: "subagent",
          system: "Você foi criado por uma proposta aprovada.",
        },
        requested_by: "build",
      },
    },
  );
  expect(created.ok()).toBeTruthy();
  const proposal = (await created.json()) as { id: string; status: string };
  expect(proposal.status).toBe("PENDING");

  const pending = await request.get(
    `${auth.baseURL}/api/proposals?workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  expect(
    ((await pending.json()) as { proposals: Array<{ id: string }> }).proposals.some(
      (item) => item.id === proposal.id,
    ),
  ).toBeTruthy();

  const before = await request.get(
    `${auth.baseURL}/api/studio/agents?scope=workspace&workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  expect(
    ((await before.json()) as Array<{ id: string }>).some(
      (item) => item.id === "agente-proposto",
    ),
  ).toBe(false);

  const resolved = await request.post(
    `${auth.baseURL}/api/proposals/${proposal.id}/resolve`,
    { headers: auth.headers, data: { approved: true } },
  );
  expect(resolved.ok()).toBeTruthy();
  expect(((await resolved.json()) as { status: string }).status).toBe("APPLIED");

  const after = await request.get(
    `${auth.baseURL}/api/studio/agents?scope=workspace&workspace_id=${workspaceId}`,
    { headers: auth.headers },
  );
  const agent = ((await after.json()) as Array<{ id: string; name?: string }>).find(
    (item) => item.id === "agente-proposto",
  );
  expect(agent?.name).toBe("Agente Proposto");

  const rejected = await request.post(
    `${auth.baseURL}/api/proposals?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        kind: "SKILL",
        action: "CREATE",
        title: "Skill rejeitada",
        payload: { id: "skill-rejeitada", body: "não deve existir" },
      },
    },
  );
  const rejectedProposal = (await rejected.json()) as { id: string };
  const denied = await request.post(
    `${auth.baseURL}/api/proposals/${rejectedProposal.id}/resolve`,
    { headers: auth.headers, data: { approved: false } },
  );
  expect(denied.ok()).toBeTruthy();
  expect(((await denied.json()) as { status: string }).status).toBe("REJECTED");
});
