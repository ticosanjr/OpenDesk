import { expect, test } from "@playwright/test";
import { createHmac } from "node:crypto";
import { api } from "./helpers";


test("io bus inbound, outbound, bindings and webhook idempotency", async ({
  request,
}) => {
  const auth = api();
  const workspaces = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  let workspaceId = workspaces[0]?.id;
  if (!workspaceId) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: { name: "IO Bus", root_path: "/tmp/opendesk-e2e/io-bus" },
    });
    workspaceId = ((await created.json()) as { id: string }).id;
  }

  const inbound = await request.post(
    `${auth.baseURL}/api/io/inbound?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        name: "Webhook E2E",
        source_type: "WEBHOOK_HTTP",
        config: { title_path: "$.ticket.subject", body_path: "$.ticket.body" },
        target_column: "TODO",
        auto_execute: false,
        is_enabled: true,
      },
    },
  );
  expect(inbound.ok()).toBeTruthy();
  const source = (await inbound.json()) as { id: string; source_type: string };
  expect(source.source_type).toBe("WEBHOOK_HTTP");

  const sink = await request.post(
    `${auth.baseURL}/api/io/outbound?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        name: "PDF E2E",
        sink_type: "DOC_COMPILER",
        config: { format: "pdf" },
        is_enabled: true,
      },
    },
  );
  expect(sink.ok()).toBeTruthy();
  const sinkBody = (await sink.json()) as { id: string };

  const createdTask = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: { title: "Tarefa IO E2E", description: "teste", tags: [] },
    },
  );
  const task = (await createdTask.json()) as { id: string };
  const bound = await request.post(`${auth.baseURL}/api/tasks/${task.id}/sinks`, {
    headers: auth.headers,
    data: { sink_ids: [sinkBody.id] },
  });
  expect(bound.ok()).toBeTruthy();
  const bindings = await request.get(`${auth.baseURL}/api/tasks/${task.id}/sinks`, {
    headers: auth.headers,
  });
  expect(((await bindings.json()) as { bindings: unknown[] }).bindings).toHaveLength(1);

  const payload = { ticket: { subject: "Chamado criado", body: "Corpo do chamado" } };
  const first = await request.post(
    `${auth.baseURL}/api/webhooks/${source.id}`,
    { data: payload },
  );
  expect(first.ok()).toBeTruthy();
  const firstTask = (await first.json()) as { task_id: string };
  const second = await request.post(
    `${auth.baseURL}/api/webhooks/${source.id}`,
    { data: payload },
  );
  const secondTask = (await second.json()) as { task_id: string };
  expect(secondTask.task_id).toBe(firstTask.task_id);

  const tools = await request.get(`${auth.baseURL}/api/io/tools`, {
    headers: auth.headers,
  });
  expect(tools.ok()).toBeTruthy();
  const toolList = (await tools.json()) as Array<{ name: string; optional: boolean }>;
  expect(toolList.some((tool) => tool.name === "typst" && tool.optional)).toBeTruthy();

  const deleted = await request.delete(
    `${auth.baseURL}/api/io/inbound/${source.id}`,
    { headers: auth.headers },
  );
  expect(deleted.ok()).toBeTruthy();
});


test("signed webhook rejects tampering and accepts a valid HMAC", async ({ request }) => {
  const auth = api();
  const workspaces = (await (
    await request.get(`${auth.baseURL}/api/workspaces`, { headers: auth.headers })
  ).json()) as Array<{ id: string }>;
  let workspaceId = workspaces[0]?.id;
  if (!workspaceId) {
    const created = await request.post(`${auth.baseURL}/api/workspaces`, {
      headers: auth.headers,
      data: { name: "HMAC", root_path: "/tmp/opendesk-e2e/hmac" },
    });
    workspaceId = ((await created.json()) as { id: string }).id;
  }
  const secret = "e2e-hmac-secret";
  const stored = await request.post(`${auth.baseURL}/api/io/secrets`, {
    headers: auth.headers,
    data: { key: "hmac-e2e", value: secret },
  });
  expect(stored.ok()).toBeTruthy();

  const source = (await (
    await request.post(`${auth.baseURL}/api/io/inbound?workspace_id=${workspaceId}`, {
      headers: auth.headers,
      data: {
        name: "HMAC Webhook",
        source_type: "WEBHOOK_HTTP",
        config: {
          secret_ref: "hmac-e2e",
          title_path: "$.title",
          body_path: "$.body",
        },
        auto_execute: false,
        is_enabled: true,
      },
    })
  ).json()) as { id: string };

  const payload = JSON.stringify({ title: "HMAC ok", body: "conteúdo" });
  const bad = await request.post(`${auth.baseURL}/api/webhooks/${source.id}`, {
    data: payload,
    headers: { "content-type": "application/json", "x-opendesk-signature": "00" },
  });
  expect(bad.status()).toBe(400);

  const signature = createHmac("sha256", secret).update(payload).digest("hex");
  const good = await request.post(`${auth.baseURL}/api/webhooks/${source.id}`, {
    data: payload,
    headers: { "content-type": "application/json", "x-opendesk-signature": signature },
  });
  expect(good.ok()).toBeTruthy();
  expect(((await good.json()) as { task_id: string }).task_id).toBeTruthy();
});
