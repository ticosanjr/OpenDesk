import { expect, test } from "@playwright/test";
import { api } from "../e2e/helpers";

const ROOT = "/tmp/opendesk-e2e/visual-baseline";
const SURFACES = [
  ["dashboard", "/#/"],
  ["kanban", "/#/kanban"],
  ["runs", "/#/runs"],
  ["packs", "/#/packs"],
  ["settings", "/#/settings"],
  ["artifacts", "/#/artifacts"],
] as const;

test("visual baselines for the main surfaces", async ({ page, request }) => {
  test.setTimeout(180_000);
  const auth = api();
  const created = await request.post(`${auth.baseURL}/api/workspaces`, {
    headers: auth.headers,
    data: {
      name: "Visual Baseline",
      root_path: ROOT,
      color_hex: "#3b82f6",
    },
  });
  expect(created.ok()).toBeTruthy();
  const workspace = (await created.json()) as { id: string };
  const activated = await request.post(
    `${auth.baseURL}/api/workspaces/${workspace.id}/activate`,
    { headers: auth.headers },
  );
  expect(activated.ok()).toBeTruthy();

  for (const [title, description, priority] of [
    ["Diagnóstico do cliente Aurora", "Analise o briefing e prepare um diagnóstico com próximos passos.", 2],
    ["Entrega do diagnóstico", "Produza o relatório executivo do diagnóstico.", 3],
    ["Revisão final", "Revise o relatório e aponte riscos antes da entrega.", 1],
  ] as const) {
    const task = await request.post(
      `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
      {
        headers: auth.headers,
        data: {
          title,
          description,
          assigned_agent: "global-agent",
          priority,
          tags: [],
        },
      },
    );
    expect(task.ok()).toBeTruthy();
  }

  const seeded = await request.post(`${auth.baseURL}/api/test/seed`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      run: {
        status: "SUCCESS",
        agent: "global-agent",
        model: "opencode-go/glm-5.3",
        duration_ms: 4200,
        tokens_input: 1200,
        tokens_output: 800,
        cost_usd: 0.042,
        summary: "Diagnóstico concluído.",
      },
      artifacts: [
        {
          file_path: "relatorio-executivo.md",
          content: "# Relatório executivo\n\nResumo da operação.",
          action_type: "CREATED",
          agent: "global-agent",
          reason: "Diagnóstico comercial",
        },
      ],
    },
  });
  expect(seeded.ok()).toBeTruthy();

  await page.emulateMedia({ reducedMotion: "reduce" });

  for (const viewport of [
    { width: 1024, height: 768 },
    { width: 1440, height: 900 },
    { width: 1920, height: 1080 },
  ]) {
    await page.setViewportSize(viewport);
    for (const [name, route] of SURFACES) {
      await page.goto(route);
      await page.waitForLoadState("networkidle");
      await page.waitForTimeout(500);
      if (name === "packs") {
        await page.getByRole("button", { name: "Recomendados" }).click();
        await page.waitForTimeout(250);
      }
      await expect(page).toHaveScreenshot(`${name}-${viewport.width}.png`, {
        fullPage: false,
      });
    }
  }
});
