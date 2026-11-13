import { formatDuration, formatNumber, formatUsd } from "./format";

export interface RunNotificationRun {
  task_title?: string | null;
  agent_name?: string | null;
  status?: string | null;
  duration_ms?: number | null;
  estimated_cost_usd?: number | null;
  tokens_input?: number | null;
  tokens_output?: number | null;
  error_message?: string | null;
}

export interface RunNotification {
  title: string;
  body: string;
}

const VERB: Record<string, string> = {
  SUCCESS: "concluída",
  DONE: "concluída",
  FAILED: "falhou",
  CANCELLED: "cancelada",
  BLOCKED: "bloqueada",
};

/** Toast/native notification text with the details worth knowing. */
export function runNotification(
  stage: string,
  fallbackTitle: string,
  run?: RunNotificationRun | null,
): RunNotification {
  const state = VERB[stage] ?? (stage === "BLOCKED" ? "bloqueada" : "encerrada");
  const name = run?.task_title?.trim() || fallbackTitle;
  const title = `Tarefa ${state} · ${name}`;
  const parts = [
    run?.agent_name ? `@${run.agent_name}` : null,
    typeof run?.duration_ms === "number"
      ? formatDuration(run.duration_ms)
      : null,
    typeof run?.estimated_cost_usd === "number"
      ? formatUsd(run.estimated_cost_usd)
      : null,
    typeof run?.tokens_input === "number" ||
    typeof run?.tokens_output === "number"
      ? `${formatNumber((run?.tokens_input ?? 0) + (run?.tokens_output ?? 0))} tokens`
      : null,
  ].filter((part): part is string => Boolean(part));
  if (stage === "FAILED" && run?.error_message?.trim()) {
    parts.push(run.error_message.trim().slice(0, 180));
  }
  return {
    title,
    body:
      parts.length > 0
        ? parts.join(" · ")
        : stage === "SUCCESS" || stage === "DONE"
          ? "A tarefa terminou com sucesso."
          : "Abra a tarefa para ver os detalhes.",
  };
}
