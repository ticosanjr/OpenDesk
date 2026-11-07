import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";

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

test("model references are validated before reaching OpenCode", async ({
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
      data: { name: "Model Workspace", root_path: "/tmp/opendesk-e2e/workspaces-model" },
    });
    workspaceId = ((await created.json()) as { id: string }).id;
  }

  const invalid = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        title: "Modelo inválido",
        description: "Não deve ser criado.",
        preferred_model: "sem-provider",
        tags: [],
      },
    },
  );
  expect(invalid.status()).toBe(400);
  expect(await invalid.text()).toContain("modelo inválido");

  const valid = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        title: "Modelo sem variante",
        description: "Deve ser aceito.",
        preferred_model: "opencode-go/glm-5.3",
        tags: [],
      },
    },
  );
  expect(valid.ok()).toBeTruthy();
  const task = (await valid.json()) as { id: string };
  expect(task.id).toBeTruthy();

  const invalidThread = await request.post(`${auth.baseURL}/api/chat/threads`, {
    headers: auth.headers,
    data: {
      workspace_id: workspaceId,
      title: "Modelo inválido",
      agent: "build",
      model: "sem-provider",
    },
  });
  expect(invalidThread.status()).toBe(400);
});
