import { expect, test } from "@playwright/test";
import { api } from "../e2e/helpers";

const ROOT = "/tmp/opendesk-e2e/visual-baseline";
const CALENDAR_DATE = "2024-01-15";

/**
 * Full route coverage at the desktop width. Dense/operational surfaces also
 * run at 1024 and 1920 to catch layout regressions the single width would miss.
 */
const ROUTES: Array<{ name: string; path: string; widths: number[] }> = [
  { name: "dashboard", path: "/#/", widths: [1024, 1440, 1920, 700, 1100] },
  { name: "search", path: "/#/search", widths: [1440] },
  { name: "kanban", path: "/#/kanban", widths: [1024, 1440, 1920, 700, 1100] },
  { name: "calendar", path: `/#/calendar?date=${CALENDAR_DATE}`, widths: [1024, 1440, 1920] },
  { name: "runs", path: "/#/runs", widths: [1024, 1440, 1920] },
  { name: "chat", path: "/#/chat", widths: [1024, 1440] },
  { name: "routines", path: "/#/routines", widths: [1024, 1440] },
  { name: "recipes", path: "/#/recipes", widths: [1024, 1440] },
  { name: "artifacts", path: "/#/artifacts", widths: [1024, 1440, 1920] },
  { name: "files", path: "/#/files", widths: [1440] },
  { name: "packs", path: "/#/packs", widths: [1024, 1440, 1920, 700, 1100] },
  { name: "studio", path: "/#/studio", widths: [1024, 1440] },
  { name: "governance", path: "/#/governance", widths: [1440] },
  { name: "observability", path: "/#/observability", widths: [1440] },
  { name: "learning", path: "/#/learning", widths: [1440] },
  { name: "router", path: "/#/router", widths: [1440] },
  { name: "integrations", path: "/#/integrations", widths: [1440] },
  { name: "settings", path: "/#/settings", widths: [1024, 1440, 1920, 700, 1100] },
];

const VIEWPORTS: Record<number, { width: number; height: number }> = {
  700: { width: 700, height: 800 },
  1024: { width: 1024, height: 768 },
  1100: { width: 1100, height: 800 },
  1440: { width: 1440, height: 900 },
  1920: { width: 1920, height: 1080 },
};

async function shot(page: import("@playwright/test").Page, name: string) {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => {
    document.querySelector<HTMLElement>(".app-main")?.scrollTo({ top: 0 });
  });
  await page.waitForTimeout(450);
  await expect(page).toHaveScreenshot(name, { fullPage: false });
}

test("visual baselines for every route and main modal", async ({ page, request }) => {
  test.setTimeout(420_000);
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

  for (const route of ROUTES) {
    for (const width of route.widths) {
      await page.setViewportSize(VIEWPORTS[width]);
      await page.goto(route.path);
      if (route.name === "packs" && width === 1440) {
        await page.getByRole("button", { name: "Recomendados" }).click();
        // Pack installation and the curated list are asynchronous; wait for
        // both so the golden does not capture a partially loaded page.
        await page
          .getByText("1 INSTALADO(S) NESTE WORKSPACE")
          .waitFor({ state: "visible", timeout: 15_000 });
        await page
          .getByRole("button", { name: "Instalar via tarefa" })
          .first()
          .waitFor({ state: "visible", timeout: 15_000 });
        await page.waitForTimeout(250);
      }
      await shot(page, `${route.name}-${width}.png`);
    }
  }

  // Modals keep their own goldens so sections, footers and scroll areas are
  // covered without duplicating every route width.
  await page.setViewportSize(VIEWPORTS[1440]);

  await page.goto("/#/kanban");
  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  await shot(page, "modal-new-task-1440.png");
  await page.keyboard.press("Escape");

  await page.goto("/#/kanban");
  await page.getByText("Diagnóstico do cliente Aurora").first().click();
  await shot(page, "modal-task-details-1440.png");
  await page.keyboard.press("Escape");

  // The routine preview lists the next runs from "now"; freeze it so the
  // golden does not change every day.
  await page.clock.install({ time: new Date("2026-11-13T12:00:00Z") });
  await page.goto("/#/routines");
  await page.getByRole("main").getByRole("button", { name: "Nova rotina" }).first().click();
  await shot(page, "modal-routine-1440.png");
  await page.keyboard.press("Escape");

  await page.goto("/#/packs");
  await page.getByRole("button", { name: /Controle OpenDesk/ }).first().click();
  await shot(page, "modal-pack-install-1440.png");
  await page.keyboard.press("Escape");

  await page.goto("/#/");
  await page.keyboard.press("Control+k");
  await shot(page, "modal-command-palette-1440.png");
});
