import type { APIRequestContext } from "@playwright/test";
import { readFileSync } from "node:fs";

const DATA_DIR = "/tmp/opendesk-e2e/data";

export function api() {
  const endpoint = JSON.parse(readFileSync(`${DATA_DIR}/api.json`, "utf8")) as {
    port: number;
    token: string;
  };
  return {
    baseURL: `http://127.0.0.1:${endpoint.port}`,
    headers: { Authorization: `Bearer ${endpoint.token}` },
  };
}

export interface SeededWorkspace {
  id: string;
  name: string;
  root: string;
}

export async function seedWorkspace(
  request: APIRequestContext,
  prefix: string,
  color = "#3b82f6",
): Promise<SeededWorkspace> {
  const suffix = `${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
  const name = `${prefix} ${suffix}`;
  const root = `/tmp/opendesk-e2e/${prefix.toLowerCase().replace(/[^a-z0-9]+/g, "-")}-${suffix}`;
  const auth = api();
  let created = await request.post(`${auth.baseURL}/api/workspaces`, {
    headers: auth.headers,
    data: { name, root_path: root, color_hex: color },
  });
  // Background cleanup from a previous UI test can briefly hold SQLite.
  for (let attempt = 0; !created.ok() && attempt < 6; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 250 * (attempt + 1)));
    created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: { name, root_path: root, color_hex: color },
    });
  }
  if (!created.ok()) {
    throw new Error(`workspace create failed: ${created.status()} ${await created.text()}`);
  }
  const workspace = (await created.json()) as { id: string };
  let activated = await request.post(
    `${auth.baseURL}/api/workspaces/${workspace.id}/activate`,
    { headers: auth.headers },
  );
  for (let attempt = 0; !activated.ok() && attempt < 4; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 200));
    activated = await request.post(
      `${auth.baseURL}/api/workspaces/${workspace.id}/activate`,
      { headers: auth.headers },
    );
  }
  if (!activated.ok()) {
    throw new Error(`workspace activate failed: ${activated.status()} ${await activated.text()}`);
  }
  return { id: workspace.id, name, root };
}

export async function installPack(
  request: APIRequestContext,
  workspaceId: string,
  packId: string,
  extra: Record<string, unknown> = {},
): Promise<void> {
  const auth = api();
  const response = await request.post(
    `${auth.baseURL}/api/starter-packs/${packId}/install`,
    {
      headers: auth.headers,
      data: { workspace_id: workspaceId, skip_conflicts: true, ...extra },
    },
  );
  if (!response.ok()) {
    throw new Error(`pack install failed: ${response.status()} ${await response.text()}`);
  }
}
