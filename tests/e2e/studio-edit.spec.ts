import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

test("studio keeps agent and skill drafts while typing and after filesystem events", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Studio Persist");
  const auth = api();
  const query = `scope=workspace&workspace_id=${workspace.id}`;

  await page.goto("/#/studio");
  await page
    .getByRole("main")
    .getByRole("button", { name: /^Workspace ·/ })
    .click();
  await page.getByRole("button", { name: "Novo agente" }).click();

  const name = page.getByLabel("Nome de exibição");
  await name.pressSequentially("Agente Persistente", { delay: 10 });
  await expect(name).toHaveValue("Agente Persistente");
  await page
    .getByLabel("Descrição")
    .fill("Descrição criada pelo teste de persistência");
  await page.getByLabel("System prompt").fill("Você é um agente persistente.");
  await page.getByRole("button", { name: "Salvar", exact: true }).click();
  await expect(
    page.getByRole("button", { name: /@agente-persistente/ }),
  ).toBeVisible();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/studio/agent?id=agente-persistente&${query}`,
        { headers: auth.headers },
      );
      return response.ok();
    })
    .toBe(true);
  let saved = await request.get(
    `${auth.baseURL}/api/studio/agent?id=agente-persistente&${query}`,
    { headers: auth.headers },
  );
  expect(saved.ok()).toBeTruthy();
  let agent = (await saved.json()) as {
    id: string;
    name?: string | null;
    system: string;
  };
  expect(agent.name).toBe("Agente Persistente");
  expect(agent.system).toContain("agente persistente");

  await name.fill("Agente Editado");
  // A workspace file change must refresh the list without overwriting the draft.
  writeFileSync(`${workspace.root}/input/watcher.txt`, "evento");
  await page.waitForTimeout(600);
  await expect(name).toHaveValue("Agente Editado");
  await page.getByRole("button", { name: "Salvar", exact: true }).click();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/studio/agent?id=agente-persistente&${query}`,
        { headers: auth.headers },
      );
      agent = (await response.json()) as typeof agent;
      return agent.name;
    })
    .toBe("Agente Editado");

  await page.getByRole("button", { name: "Skills", exact: true }).click();
  await page.getByRole("button", { name: "Nova skill" }).click();
  const skillName = page.getByLabel("Nome de exibição");
  await skillName.pressSequentially("Skill Persistente", { delay: 10 });
  await expect(skillName).toHaveValue("Skill Persistente");
  await page.getByLabel("Descrição").fill("Skill criada no E2E");
  await page
    .getByLabel("Instruções (SKILL.md)")
    .fill("## Quando usar\n\nSempre que precisar persistir.");
  await page.getByRole("button", { name: "Salvar", exact: true }).click();

  await expect
    .poll(async () => {
      const response = await request.get(
        `${auth.baseURL}/api/studio/skill?id=skill-persistente&${query}`,
        { headers: auth.headers },
      );
      return response.ok();
    })
    .toBe(true);
  saved = await request.get(
    `${auth.baseURL}/api/studio/skill?id=skill-persistente&${query}`,
    { headers: auth.headers },
  );
  expect(saved.ok()).toBeTruthy();
  const skill = (await saved.json()) as { name?: string | null; body: string };
  expect(skill.name).toBe("Skill Persistente");
  expect(skill.body).toContain("persistir");
});
