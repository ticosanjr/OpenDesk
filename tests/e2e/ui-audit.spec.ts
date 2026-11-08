import { expect, test, type Page } from "@playwright/test";
import { api, installPack, seedWorkspace } from "./helpers";

const ROUTES = [
  "/",
  "/search",
  "/kanban",
  "/calendar",
  "/runs",
  "/chat",
  "/routines",
  "/recipes",
  "/artifacts",
  "/files",
  "/packs",
  "/studio",
  "/router",
  "/integrations",
  "/settings",
];

interface AuditIssue {
  kind: string;
  detail: string;
}

async function audit(page: Page, scope = "body"): Promise<AuditIssue[]> {
  return page.evaluate((selector) => {
    const root = document.querySelector(selector) ?? document.body;
    const issues: AuditIssue[] = [];
    const visible = (element: Element) => {
      const node = element as HTMLElement;
      const rect = node.getBoundingClientRect();
      const style = getComputedStyle(node);
      return (
        rect.width > 1 &&
        rect.height > 1 &&
        style.display !== "none" &&
        style.visibility !== "hidden" &&
        style.opacity !== "0" &&
        style.pointerEvents !== "none"
      );
    };
    const inHorizontalScroller = (element: Element) => {
      let parent = element.parentElement;
      while (parent) {
        const style = getComputedStyle(parent);
        if (/(auto|scroll)/.test(style.overflowX)) return true;
        parent = parent.parentElement;
      }
      return false;
    };
    const actionable = Array.from(
      root.querySelectorAll(
        'button, a[href], input, select, textarea, [role="button"]',
      ),
    ).filter(visible);

    for (const element of actionable) {
      const node = element as HTMLElement;
      const rect = node.getBoundingClientRect();
      const label =
        (node.getAttribute("aria-label") || node.textContent || node.tagName)
          .trim()
          .replace(/\s+/g, " ")
          .slice(0, 70);

      if (
        node.tagName === "BUTTON" &&
        !node.textContent?.trim() &&
        !node.getAttribute("aria-label") &&
        !node.getAttribute("title")
      ) {
        issues.push({ kind: "unnamed-button", detail: `<button> sem nome em ${selector}` });
      }

      if (!inHorizontalScroller(node)) {
        if (rect.left < -2 || rect.right > window.innerWidth + 2) {
          issues.push({
            kind: "horizontal-overflow",
            detail: `${label} em ${Math.round(rect.left)}..${Math.round(rect.right)} (viewport ${window.innerWidth})`,
          });
        }
      }

      const clickableCenter = () => {
        const current = node.getBoundingClientRect();
        return {
          x: current.left + current.width / 2,
          y: current.top + current.height / 2,
        };
      };
      let center = clickableCenter();
      if (
        center.x > 0 &&
        center.x < window.innerWidth &&
        center.y > 0 &&
        center.y < window.innerHeight &&
        !node.hasAttribute("disabled")
      ) {
        let hit = document.elementFromPoint(center.x, center.y);
        if (hit && !(node === hit || node.contains(hit))) {
          // Give scrollable containers a chance to reveal the control before
          // treating it as genuinely covered by another element.
          node.scrollIntoView({ block: "nearest", inline: "nearest" });
          center = clickableCenter();
          hit = document.elementFromPoint(center.x, center.y);
          if (hit && !(node === hit || node.contains(hit))) {
            issues.push({
              kind: "covered",
              detail: `${label} coberto por ${hit.tagName.toLowerCase()}${hit.className ? `.${String(hit.className).split(" ")[0]}` : ""}`,
            });
          }
        }
      }
    }

    const main = document.querySelector(".app-main");
    if (main && main.scrollWidth > main.clientWidth + 4) {
      issues.push({
        kind: "main-overflow",
        detail: `app-main ${main.scrollWidth} > ${main.clientWidth}`,
      });
    }
    return issues;
  }, scope);
}

test("all main routes are reachable and keep interactive elements usable", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "UI Audit");
  await installPack(request, workspace.id, "essenciais");

  await request.post(`${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`, {
    headers: auth.headers,
    data: {
      title:
        "Tarefa com título muito longo para validar quebra de linha e posicionamento dos cards no Kanban",
      description:
        "Descrição longa para validar truncamento, altura consistente e ações visíveis sem sobreposição. ".repeat(3),
      execution_mode: "AUTO_DSH",
      tags: ["audit", "layout"],
    },
  });
  const done = await request.post(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        title: "Tarefa concluída para retenção",
        description: "Concluída recentemente.",
        execution_mode: "AUTO_DSH",
        tags: [],
      },
    },
  );
  const doneTask = (await done.json()) as { id: string };
  await request.post(`${auth.baseURL}/api/tasks/${doneTask.id}/transition`, {
    headers: auth.headers,
    data: { status: "DONE" },
  });
  await request.put(
    `${auth.baseURL}/api/recipes/audit-recipe?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        yaml: `id: audit-recipe
name: Receita de auditoria
enabled: true
triggers: []
flow:
  - type: step
    id: passo-1
    name: Passo de auditoria
    mode: build
    prompt: ok
`,
      },
    },
  );
  await request.post(`${auth.baseURL}/api/schedules`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      name: "Rotina de auditoria",
      description: "Rotina para validar a lista.",
      cron_expression: "0 8 * * *",
      timezone: "America/Sao_Paulo",
      agent_name: null,
      command_or_prompt: "Resumo diário",
      trigger_type: "CRON",
      trigger_config_json: "{}",
      target_kind: "PROMPT",
      recipe_id: null,
      router_policy: "COST_BENEFIT",
      output_folder_template: "output/{YYYY}-{MM}/{schedule_slug}",
      notify_email_recipients: null,
      notify_signal: false,
      notify_signal_recipients: null,
      notify_only_on_change: false,
      recurrence_json: null,
      one_time: false,
      misfire_policy: "SKIP_TO_NEXT",
      safe_readonly_mode: true,
      sync_to_google_docs: false,
      sync_to_google_calendar: false,
      is_active: false,
    },
  });

  for (const viewport of [
    { width: 1440, height: 900 },
    { width: 1100, height: 800 },
  ]) {
    await page.setViewportSize(viewport);
    for (const route of ROUTES) {
      const uncaught: string[] = [];
      const onError = (error: Error) => uncaught.push(error.message);
      page.on("pageerror", onError);
      await page.goto(`/#${route}`);
      await page.waitForTimeout(450);
      const issues = await audit(page);
      page.off("pageerror", onError);
      expect(
        issues,
        `${route} @ ${viewport.width}x${viewport.height}: ${issues
          .map((issue) => `${issue.kind}: ${issue.detail}`)
          .join(" | ")}`,
      ).toEqual([]);
      expect(uncaught, `${route} uncaught errors`).toEqual([]);
    }
  }

});

test("key modals keep buttons visible and reachable", async ({ page, request }) => {
  const workspace = await seedWorkspace(request, "UI Audit Modal");
  await installPack(request, workspace.id, "essenciais");

  await page.setViewportSize({ width: 1100, height: 800 });
  await page.goto("/#/kanban");
  await page.getByRole("main").getByRole("button", { name: "Nova tarefa" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await audit(page, '[role="dialog"]')).toEqual([]);
  await page.getByRole("button", { name: "Cancelar" }).first().click();

  await page.goto("/#/packs");
  await page.getByRole("button", { name: /Essenciais/ }).first().click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await audit(page, '[role="dialog"]')).toEqual([]);
  await page.getByRole("dialog").getByRole("button", { name: "Fechar" }).click();

  await page.goto("/#/routines");
  await page.getByRole("main").getByRole("button", { name: "Nova rotina" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await audit(page, '[role="dialog"]')).toEqual([]);
  await page.getByRole("dialog").getByRole("button", { name: "Cancelar" }).click();

  await page.goto("/#/recipes");
  await page.getByRole("button", { name: /Dry-run/ }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await audit(page, '[role="dialog"]')).toEqual([]);
  await page.getByRole("dialog").getByRole("button", { name: "Cancelar" }).click();

  const auth = api();
});
