import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

function localInput(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`;
}

test("calendar creates, edits, filters and deletes local events", async ({
  page,
  request,
}) => {
  const auth = api();
  const workspace = await seedWorkspace(request, "Calendar");

  const schedule = await request.post(`${auth.baseURL}/api/schedules`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      name: "Rotina Calendário",
      command_or_prompt: "Executar",
      cron_expression: "0 9 * * *",
      timezone: "UTC",
    },
  });
  expect(schedule.ok()).toBeTruthy();

  await page.goto("/#/calendar");
  await expect(page.getByRole("heading", { name: "Calendário" })).toBeVisible();
  await expect(page.getByText("Rotina Calendário").first()).toBeVisible();

  // Without OAuth the import dialog stays actionable and points to Integrações.
  await page.getByRole("main").getByRole("button", { name: "Google" }).click();
  await expect(page.getByText(/reconecte/i)).toBeVisible();
  await page.getByRole("button", { name: "Fechar", exact: true }).last().click();

  const start = new Date();
  start.setHours(12, 0, 0, 0);
  const end = new Date(start.getTime() + 60 * 60 * 1000);

  await page.getByRole("main").getByRole("button", { name: "Novo evento" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Título", { exact: true }).fill("Evento E2E");
  await dialog.getByLabel("Descrição").fill("Contexto do evento");
  await dialog.getByRole("textbox", { name: "Local", exact: true }).fill("Sala 1");
  await dialog.getByLabel("Início", { exact: true }).fill(localInput(start));
  await dialog.getByLabel("Fim", { exact: true }).fill(localInput(end));
  await dialog.getByRole("button", { name: "Criar evento" }).click();
  await expect(page.getByText("Evento E2E").first()).toBeVisible();

  await page.getByRole("button", { name: /Evento E2E/ }).first().click();
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Título", { exact: true }).fill("Evento E2E editado");
  await dialog.getByRole("button", { name: "Salvar", exact: true }).click();
  await expect(page.getByText("Evento E2E editado").first()).toBeVisible();

  const events = (await (
    await request.get(
      `${auth.baseURL}/api/calendar/events?workspace_id=${workspace.id}`,
      { headers: auth.headers },
    )
  ).json()) as Array<{ id: string; title: string }>;
  expect(events.some((event) => event.title === "Evento E2E editado")).toBe(true);

  // Category filters hide and restore the event.
  const eventsFilter = page.getByRole("button", { name: "Eventos", exact: true });
  const eventButtons = page.getByRole("button", { name: /Evento E2E editado/ });
  await eventsFilter.click();
  await expect(eventButtons).toHaveCount(0);
  await eventsFilter.click();
  await expect(eventButtons.first()).toBeVisible();

  await page.getByRole("button", { name: /Evento E2E editado/ }).first().click();
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Excluir", exact: true }).click();
  const confirm = page.getByRole("dialog").last();
  await confirm.getByRole("button", { name: "Excluir evento" }).click();
  await expect(eventButtons).toHaveCount(0);

  const after = (await (
    await request.get(
      `${auth.baseURL}/api/calendar/events?workspace_id=${workspace.id}`,
      { headers: auth.headers },
    )
  ).json()) as Array<{ id: string }>;
  expect(after).toHaveLength(0);
});
