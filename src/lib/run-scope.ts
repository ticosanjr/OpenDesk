import type { TaskRun } from "@/lib/types";

export type RunScope = "work" | "routine" | "recipe" | "automation" | "chat";

export const RUN_SCOPE_LABEL: Record<RunScope, string> = {
  work: "trabalho",
  routine: "rotina",
  recipe: "receita",
  automation: "automação",
  chat: "conversa",
};

export const RUN_SCOPE_OPTIONS: Array<[string, string]> = [
  ["", "tudo"],
  ["work", "trabalho"],
  ["automations", "automações"],
  ["routine", "rotinas"],
  ["recipe", "receitas"],
  ["chat", "conversas"],
];

export function runScope(
  run: Pick<TaskRun, "task_origin" | "task_id" | "schedule_id" | "source">,
): RunScope {
  if (run.source === "chat") return "chat";
  const origin = run.task_origin ?? "";
  if (origin === "RECIPE") return "recipe";
  if (origin === "ROUTINE") return "routine";
  if (origin === "IO_BUS" || origin === "DELEGATION") return "automation";
  if (!run.task_id && run.schedule_id) return "routine";
  return "work";
}

export function matchesRunScope(run: TaskRun, scope: string): boolean {
  if (!scope) return true;
  const value = runScope(run);
  if (scope === "automations") return value !== "work" && value !== "chat";
  if (scope === "work") return value === "work";
  return value === scope;
}
