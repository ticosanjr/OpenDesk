import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("memory scope filters and inline switches separate normal and chat", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Memory Scope");
  for (const [title, scope] of [
    ["Nota Normal", "normal"],
    ["Nota Chat", "chat"],
    ["Nota Ambos", "both"],
  ]) {
    const response = await request.post(
      `${auth.baseURL}/api/memory/notes?workspace_id=${workspace.id}`,
      {
        headers: auth.headers,
        data: {
          namespace: "core",
          title,
          body: `${title} no prompt`,
          usage_scope: scope,
        },
      },
    );
    expect(response.ok()).toBeTruthy();
  }
  for (const [name, scope] of [
    ["Regra Normal", "normal"],
    ["Regra Chat", "chat"],
    ["Regra Ambos", "both"],
  ]) {
    const response = await request.post(
      `${auth.baseURL}/api/personal-context?workspace_id=${workspace.id}`,
      {
        headers: auth.headers,
        data: {
          scope: "workspace",
          workspace_id: workspace.id,
          name,
          instructions: `${name} pessoal`,
          is_enabled: true,
          usage_scope: scope,
        },
      },
    );
    expect(response.ok()).toBeTruthy();
  }

  await page.goto("/#/learning?tab=memory");
  await expect(page.getByText("Nota Normal", { exact: true })).toBeVisible();
  await expect(page.getByText("Nota Chat", { exact: true })).toBeVisible();
  await expect(page.getByText("Nota Ambos", { exact: true })).toBeVisible();

  const memoryFilter = page.getByRole("group", {
    name: "Filtrar por escopo de memória",
  });
  await memoryFilter.getByRole("button", { name: "Chat", exact: true }).click();
  await expect(page.getByText("Nota Normal", { exact: true })).toHaveCount(0);
  await expect(page.getByText("Nota Chat", { exact: true })).toBeVisible();
  await expect(page.getByText("Nota Ambos", { exact: true })).toHaveCount(0);

  await page
    .getByRole("combobox", { name: "Escopo de memória de Nota Chat" })
    .selectOption("both");
  await expect(page.getByText("Escopo alterado para ambos").first()).toBeVisible();
  await memoryFilter.getByRole("button", { name: "Ambos", exact: true }).click();
  await expect(page.getByText("Nota Chat", { exact: true })).toBeVisible();
  await expect(page.getByText("Nota Ambos", { exact: true })).toBeVisible();

  await page.goto("/#/learning?tab=context");
  await expect(page.getByText("Regra Normal pessoal")).toBeVisible();
  const contextFilter = page.getByRole("group", {
    name: "Filtrar por escopo de memória",
  });
  await contextFilter.getByRole("button", { name: "Normal", exact: true }).click();
  await expect(page.getByText("Regra Normal pessoal")).toBeVisible();
  await expect(page.getByText("Regra Chat pessoal")).toHaveCount(0);
  await expect(page.getByText("Regra Ambos pessoal")).toHaveCount(0);
});

test("MCP memory scope tools update and reject invalid values", async ({ request }) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Memory Scope MCP");
  const noteResponse = await request.post(
    `${auth.baseURL}/api/memory/notes?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: { namespace: "core", title: "MCP", body: "Escopo" },
    },
  );
  const note = (await noteResponse.json()) as { id: string };
  const groupResponse = await request.post(
    `${auth.baseURL}/api/personal-context?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        scope: "workspace",
        workspace_id: workspace.id,
        name: "MCP pessoal",
        instructions: "Escopo pessoal",
        is_enabled: true,
      },
    },
  );
  const group = (await groupResponse.json()) as { id: string };
  const mcp = `${auth.baseURL}/api/mcp?workspace_id=${workspace.id}`;

  const callTool = (id: number, name: string, args: Record<string, unknown>) =>
    request.post(mcp, {
      headers: auth.headers,
      data: {
        jsonrpc: "2.0",
        id,
        method: "tools/call",
        params: { name, arguments: args },
      },
    });

  const noteScope = await callTool(1, "memory_set_scope", {
    id: note.id,
    usage_scope: "chat",
  });
  expect(noteScope.ok()).toBeTruthy();
  const noteBody = (await noteScope.json()) as { error?: unknown };
  expect(noteBody.error).toBeUndefined();

  const contextScope = await callTool(2, "personal_context_set_scope", {
    id: group.id,
    usage_scope: "both",
  });
  expect(contextScope.ok()).toBeTruthy();
  const contextBody = (await contextScope.json()) as { error?: unknown };
  expect(contextBody.error).toBeUndefined();

  const invalid = await callTool(3, "personal_context_set_scope", {
    id: group.id,
    usage_scope: "invalido",
  });
  const invalidBody = (await invalid.json()) as { error?: { code?: number } };
  expect(invalidBody.error?.code).toBe(-32602);

  const notes = await request.get(
    `${auth.baseURL}/api/memory/notes?workspace_id=${workspace.id}`,
    { headers: auth.headers },
  );
  const list = (await notes.json()) as Array<{ usage_scope: string }>;
  expect(list[0]?.usage_scope).toBe("chat");
});
