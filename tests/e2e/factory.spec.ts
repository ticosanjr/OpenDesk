import { expect, test } from "@playwright/test";
import { api } from "./helpers";

test("default factory workspace ships with capability tooling", async ({
  request,
}) => {
  const auth = api();
  const workspaces = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string; name: string; root_path: string }>;
  const factory = workspaces.find(
    (workspace) => workspace.name === "Fábrica de Capacidades",
  );
  expect(factory).toBeTruthy();
  const workspaceId = factory!.id;

  const agents = (await (
    await request.get(
      `${auth.baseURL}/api/studio/agents?scope=workspace&workspace_id=${workspaceId}`,
      { headers: auth.headers },
    )
  ).json()) as Array<{ id: string }>;
  expect(agents.some((agent) => agent.id === "arquiteto-capacidades")).toBe(true);

  const skills = (await (
    await request.get(
      `${auth.baseURL}/api/studio/skills?scope=workspace&workspace_id=${workspaceId}`,
      { headers: auth.headers },
    )
  ).json()) as Array<{ id: string }>;
  expect(skills.some((skill) => skill.id === "criacao-de-skills")).toBe(true);

  const commands = (await (
    await request.get(
      `${auth.baseURL}/api/studio/commands?scope=workspace&workspace_id=${workspaceId}`,
      { headers: auth.headers },
    )
  ).json()) as Array<{ id: string }>;
  expect(commands.some((command) => command.id === "criar-skill")).toBe(true);

  const recipes = (await (
    await request.get(
      `${auth.baseURL}/api/recipes?workspace_id=${workspaceId}`,
      { headers: auth.headers },
    )
  ).json()) as { recipes: Array<{ id: string }> };
  expect(
    recipes.recipes.some((recipe) => recipe.id === "criar-skill-alto-nivel"),
  ).toBe(true);
});
