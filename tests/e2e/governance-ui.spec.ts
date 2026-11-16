import { expect, test, type APIRequestContext } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

async function createAgent(
  request: APIRequestContext,
  workspaceId: string,
  id: string,
  name: string,
) {
  const auth = api();
  const response = await request.put(
    `${auth.baseURL}/api/studio/agent?id=${id}&scope=workspace&workspace_id=${workspaceId}`,
    {
      headers: auth.headers,
      data: {
        name,
        description: `Agente ${name}`,
        mode: "subagent",
        system: `Você é ${name}.`,
      },
    },
  );
  expect(response.ok()).toBeTruthy();
}

test("governance edits profiles, creates goals and delegates with approval", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Governance UI");
  const auth = api();
  await createAgent(request, workspace.id, "lead", "Líder");
  await createAgent(request, workspace.id, "build", "Executor");

  await page.goto("/#/governance");
  await expect(
    page.getByRole("heading", { name: "Agentes & Governança" }),
  ).toBeVisible();
  await expect(page.getByText("@lead").first()).toBeVisible();

  const leadRow = page.locator('[data-agent-id="lead"]');
  await leadRow.getByRole("button", { name: "Editar" }).click();
  const profile = page.getByRole("dialog");
  await profile.getByLabel("Cargo / papel").fill("Líder de entrega");
  await profile.getByLabel("Time").fill("Operações");
  await profile.getByLabel("Budget mensal (US$)").fill("50");
  await profile.getByLabel("Quota de concorrência").fill("2");
  await profile.getByRole("switch", { name: "Heartbeat autônomo" }).click();
  await profile.getByLabel("Intervalo do heartbeat (min)").fill("30");
  await profile.getByRole("button", { name: "Salvar governança" }).click();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/governance?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as {
        profiles: Array<{
          profile: {
            agent_id: string;
            role?: string | null;
            monthly_budget_usd?: number | null;
            heartbeat_enabled: boolean;
          };
        }>;
      };
      return body.profiles.find((row) => row.profile.agent_id === "lead")?.profile;
    })
    .toMatchObject({
      role: "Líder de entrega",
      monthly_budget_usd: 50,
      heartbeat_enabled: true,
    });

  await page.getByRole("button", { name: "Nova meta" }).first().click();
  const goal = page.getByRole("dialog");
  await goal.getByLabel("Agente").selectOption("lead");
  await goal.getByLabel("Título").fill("Publicar 4 relatórios");
  await goal.getByLabel("Métrica").fill("relatórios");
  await goal.getByLabel("Meta").fill("4");
  await goal.getByLabel("Atual").fill("1");
  await goal.getByRole("button", { name: "Criar meta" }).click();
  await expect(page.getByText("Publicar 4 relatórios").first()).toBeVisible();

  const delegations = page
    .locator("section")
    .filter({ hasText: "Delegações" })
    .filter({ hasText: "agente → agente" })
    .first();
  await delegations
    .getByRole("combobox", { name: "De", exact: true })
    .selectOption("lead");
  await delegations
    .getByRole("combobox", { name: "Para", exact: true })
    .selectOption("build");
  await delegations
    .getByRole("textbox", { name: "Título", exact: true })
    .fill("Revisar entregáveis");
  await delegations
    .getByRole("textbox", { name: "Prompt", exact: true })
    .fill("Revise os entregáveis e proponha ajustes.");
  await page.getByRole("button", { name: "Solicitar delegação" }).click();
  await expect(page.getByText("Revisar entregáveis").first()).toBeVisible();
  await page.getByRole("button", { name: "Aprovar e executar" }).click();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/governance?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const body = (await response.json()) as {
        delegations: Array<{ title: string; status: string; task_id?: string | null }>;
      };
      return body.delegations.find((item) => item.title === "Revisar entregáveis");
    })
    .toMatchObject({ status: "APPROVED" });

  const tasks = await request.get(
    `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
    { headers: auth.headers },
  );
  const taskList = (await tasks.json()) as Array<{
    title: string;
    origin: string;
  }>;
  expect(
    taskList.some(
      (task) => task.title === "[Delegação] Revisar entregáveis" && task.origin === "DELEGATION",
    ),
  ).toBe(true);
});

test("governance heartbeat creates a task without executing when the engine is offline", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Governance Heartbeat");
  const auth = api();
  await createAgent(request, workspace.id, "lead", "Líder");
  await request.put(
    `${auth.baseURL}/api/governance/agents/lead?workspace_id=${workspace.id}`,
    {
      headers: auth.headers,
      data: {
        role: "Líder",
        heartbeat_enabled: true,
        heartbeat_minutes: 30,
      },
    },
  );

  await page.goto("/#/governance");
  await page.getByRole("button", { name: "Check-in" }).first().click();
  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/tasks?workspace_id=${workspace.id}`,
        { headers: auth.headers },
      );
      const tasks = (await response.json()) as Array<{ title: string; origin: string }>;
      return tasks.some(
        (task) => task.title === "Heartbeat · @lead" && task.origin === "ROUTINE",
      );
    })
    .toBe(true);
});
