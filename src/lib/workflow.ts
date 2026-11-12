export type WorkflowTone = "default" | "blue" | "green" | "yellow" | "red" | "purple";

export interface WorkflowLike {
  status: string;
  phase_status?: string | null;
  held?: boolean;
  workflow_state?: string | null;
  workflow_reason?: string | null;
}

export interface WorkflowMeta {
  state: string;
  label: string;
  tone: WorkflowTone;
  reason: string;
  description: string;
  tooltip: string;
}

const STATE_META: Record<string, { label: string; tone: WorkflowTone; description: string }> = {
  READY: {
    label: "pronta",
    tone: "default",
    description: "Na fila, aguardando o motor ou um worker livre. Inicia automaticamente.",
  },
  QUEUED: {
    label: "na fila",
    tone: "yellow",
    description: "Enfileirada e aguardando um worker ou o motor ficar online.",
  },
  HELD: {
    label: "em espera",
    tone: "default",
    description: "Não inicia automaticamente até você retomar.",
  },
  RUNNING: {
    label: "executando",
    tone: "blue",
    description: "Há uma execução ativa agora.",
  },
  NEEDS_APPROVAL: {
    label: "aprovar plano",
    tone: "yellow",
    description: "A execução pausou esperando aprovação do plano.",
  },
  WAITING_INPUT: {
    label: "aguardando você",
    tone: "yellow",
    description: "O agente precisa de uma resposta ou permissão para continuar.",
  },
  BLOCKED: {
    label: "atenção",
    tone: "red",
    description: "A tarefa precisa de intervenção antes de continuar.",
  },
  DONE: {
    label: "concluída",
    tone: "green",
    description: "Execução concluída.",
  },
  CANCELLED: {
    label: "cancelada",
    tone: "default",
    description: "A execução foi interrompida.",
  },
};

const REASON_META: Record<string, { label: string; description: string }> = {
  ready: {
    label: "pronta para executar",
    description: "A tarefa está na fila e será iniciada automaticamente.",
  },
  queued: {
    label: "na fila",
    description: "Aguardando um worker ou o motor ficar online.",
  },
  held: {
    label: "em espera",
    description: "A tarefa foi colocada em espera e não inicia sozinha.",
  },
  approval: {
    label: "aprovação de plano",
    description: "Revise o plano e aprove para iniciar o Build.",
  },
  hitl: {
    label: "pergunta do agente",
    description: "O agente fez uma pergunta e a execução está pausada.",
  },
  permission: {
    label: "permissão necessária",
    description: "O agente pediu acesso a um recurso para continuar.",
  },
  failed: {
    label: "falhou",
    description: "A última execução falhou. Abra o diagnóstico para ver a causa.",
  },
  manual_block: {
    label: "bloqueada manualmente",
    description: "A tarefa foi bloqueada por uma pessoa.",
  },
  in_progress_without_run: {
    label: "sem execução ativa",
    description:
      "A tarefa está em execução, mas não há job/run ativo. Execute novamente ou mova para a fila.",
  },
  done: {
    label: "concluída",
    description: "A execução terminou com sucesso.",
  },
  cancelled: {
    label: "cancelada",
    description: "A execução foi cancelada.",
  },
};

export function fallbackWorkflow(task: WorkflowLike): { state: string; reason?: string } {
  const phase = task.phase_status ?? "";
  if (task.status === "CANCELLED") return { state: "CANCELLED", reason: "cancelled" };
  if (task.status === "DONE") return { state: "DONE", reason: "done" };
  if (task.status === "BLOCKED" || phase.includes("failed")) {
    return { state: "BLOCKED", reason: phase.includes("failed") ? "failed" : "manual_block" };
  }
  if (phase === "awaiting_approval") return { state: "NEEDS_APPROVAL", reason: "approval" };
  if (phase === "awaiting_permission") return { state: "WAITING_INPUT", reason: "permission" };
  if (phase === "awaiting_input" || phase === "waiting_hitl") {
    return { state: "WAITING_INPUT", reason: "hitl" };
  }
  if (task.status === "IN_PROGRESS") {
    return phase === "running"
      ? { state: "RUNNING" }
      : { state: "BLOCKED", reason: "in_progress_without_run" };
  }
  if (task.status === "IN_REVIEW") return { state: "WAITING_INPUT", reason: "hitl" };
  if (task.held || phase === "held") return { state: "HELD", reason: "held" };
  if (phase === "queued" || phase === "follow_up_queued") {
    return { state: "QUEUED", reason: "queued" };
  }
  return { state: "READY", reason: "ready" };
}

export function workflowMeta(task: WorkflowLike): WorkflowMeta {
  const fallback = fallbackWorkflow(task);
  const state = task.workflow_state || fallback.state;
  const stateMeta = STATE_META[state] ?? STATE_META.READY;
  const reasonCode = task.workflow_reason || fallback.reason;
  const reasonMeta = reasonCode ? REASON_META[reasonCode] : undefined;
  const reason = reasonMeta?.label ?? reasonCode ?? stateMeta.label;
  const description = reasonMeta?.description ?? stateMeta.description;
  return {
    state,
    label: stateMeta.label,
    tone: stateMeta.tone,
    reason,
    description,
    tooltip: `${stateMeta.label}: ${description}`,
  };
}
