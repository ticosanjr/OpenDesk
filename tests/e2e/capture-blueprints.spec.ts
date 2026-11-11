import { expect, test } from "@playwright/test";
import { existsSync, readdirSync } from "node:fs";
import { api } from "./helpers";


test("quick capture, blueprints and model-pinned fork", async ({ request }) => {
  const auth = api();
  const existing = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string; root_path: string }>;
  let workspace = existing[0];
  if (!workspace) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: {
        name: "Capture Workspace",
        root_path: "/tmp/opendesk-e2e/workspaces-capture",
      },
    });
    workspace = (await created.json()) as { id: string; root_path: string };
  }

  const blueprints = (await (
    await request.get(`${auth.baseURL}/api/blueprints`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  expect(blueprints.length).toBeGreaterThanOrEqual(3);

  const applied = await request.post(
    `${auth.baseURL}/api/blueprints/engenharia/apply?workspace_id=${workspace.id}`,
    { headers: auth.headers },
  );
  expect(applied.ok()).toBeTruthy();
  expect(existsSync(`${workspace.root_path}/.opencode/agents/implementador.md`)).toBeTruthy();
  expect(existsSync(`${workspace.root_path}/.opencode/skills/revisao/SKILL.md`)).toBeTruthy();

  const captured = await request.post(`${auth.baseURL}/api/capture`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      content: "https://example.com/ideia\nRevisar este link depois.",
    },
  });
  expect(captured.ok()).toBeTruthy();
  const task = (await captured.json()) as { id: string; tags_json: string };
  expect(task.tags_json).toContain("quick-capture");

  const fork = await request.post(`${auth.baseURL}/api/tasks/${task.id}/fork`, {
    headers: auth.headers,
  });
  expect(fork.ok()).toBeTruthy();
  const forked = (await fork.json()) as { title: string; forked_from_task_id: string };
  expect(forked.title).toContain("fork");
  expect(forked.forked_from_task_id).toBe(task.id);

  const agents = (await (
    await request.get(
      `${auth.baseURL}/api/agents/states?workspace_id=${workspace.id}`,
      { headers: auth.headers },
    )
  ).json()) as { agents: Array<{ agent: string; state: string }> };
  expect(Array.isArray(agents.agents)).toBeTruthy();

  // The blueprint must not overwrite what already exists.
  const agentsDir = `${workspace.root_path}/.opencode/agents`;
  const before = readdirSync(agentsDir).length;
  const again = await request.post(
    `${auth.baseURL}/api/blueprints/engenharia/apply?workspace_id=${workspace.id}`,
    { headers: auth.headers },
  );
  expect(((await again.json()) as { created: string[] }).created).toHaveLength(0);
  expect(readdirSync(agentsDir).length).toBe(before);
});
