import type { Dictionary, FlowEntry } from "../recipe-types";

export const HITL_TYPES = [
  {
    value: "BINARY_GATE",
    label: "Aprovação simples (BINARY_GATE)",
    description: "Uma decisão de aprovar ou rejeitar.",
  },
  {
    value: "MULTI_CHOICE",
    label: "Escolha uma opção (MULTI_CHOICE)",
    description: "O usuário escolhe uma das opções informadas.",
  },
  {
    value: "APPROVAL_WITH_FEEDBACK",
    label: "Aprovar com feedback (APPROVAL_WITH_FEEDBACK)",
    description: "Aprova, rejeita ou pede uma correção com texto.",
  },
  {
    value: "ARTIFACT_EDIT_HANDOFF",
    label: "Editar arquivo (ARTIFACT_EDIT_HANDOFF)",
    description: "O usuário edita o artefato e devolve o arquivo.",
  },
  {
    value: "TOOL_PERMISSION_PROXY",
    label: "Permissão de ferramenta (TOOL_PERMISSION_PROXY)",
    description: "Autoriza uma ferramenta externa antes de continuar.",
  },
];

export const SINK_TYPES = [
  { value: "DOC_COMPILER", label: "Compilar documento (DOC_COMPILER)" },
  { value: "EMAIL_SMTP", label: "Enviar e-mail (EMAIL_SMTP)" },
  { value: "SIGNAL_MSG", label: "Enviar Signal (SIGNAL_MSG)" },
  { value: "TELEGRAM_MSG", label: "Enviar Telegram (TELEGRAM_MSG)" },
  { value: "RSYNC_FS", label: "Copiar via Rsync (RSYNC_FS)" },
  { value: "GIT_COMMIT", label: "Commit no Git (GIT_COMMIT)" },
  { value: "SQL_INSERT", label: "Inserir no SQL (SQL_INSERT)" },
];

export const CHANNELS = ["DESKTOP_UI", "EMAIL", "SIGNAL", "TELEGRAM"];

export const ON_TIMEOUT = [
  { value: "PAUSE_AND_ALERT", label: "Pausar e avisar" },
  { value: "AUTO_APPROVE", label: "Aprovar automaticamente" },
  { value: "ABORT", label: "Abortar a receita" },
];

function isFlowEntry(value: unknown): value is FlowEntry {
  if (!value || typeof value !== "object") return false;
  const type = (value as Dictionary).type;
  return type === "step" || type === "hitl" || type === "sink";
}

export function normalizeFlow(recipe: Dictionary): FlowEntry[] {
  if (Array.isArray(recipe.flow)) {
    const flow = recipe.flow.filter(isFlowEntry);
    if (flow.length > 0) return flow;
  }
  const steps = Array.isArray(recipe.steps)
    ? (recipe.steps as Dictionary[])
    : [];
  const checkpoints = Array.isArray(recipe.hitl_checkpoints)
    ? (recipe.hitl_checkpoints as Dictionary[])
    : [];
  const sinks = Array.isArray(recipe.sinks)
    ? (recipe.sinks as Dictionary[])
    : [];
  const flow: FlowEntry[] = [];
  for (const step of steps) {
    flow.push({ ...step, type: "step" });
    for (const checkpoint of checkpoints.filter(
      (item) => item.after_step === step.id,
    )) {
      flow.push({ ...checkpoint, type: "hitl" });
    }
  }
  for (const checkpoint of checkpoints.filter(
    (item) => !item.after_step,
  )) {
    flow.push({ ...checkpoint, type: "hitl" });
  }
  for (const sink of sinks) {
    flow.push({
      ...sink,
      type: "sink",
      sink_type: sink.sink_type ?? sink.type,
    });
  }
  return flow;
}

export function normalizeTriggers(recipe: Dictionary): Dictionary[] {
  if (Array.isArray(recipe.triggers)) {
    return (recipe.triggers as Dictionary[]).filter(
      (trigger) => trigger && typeof trigger === "object",
    );
  }
  const legacy = recipe.trigger;
  if (legacy && typeof legacy === "object") {
    const trigger = legacy as Dictionary;
    if (typeof trigger.type === "string" && trigger.type) return [trigger];
  }
  return [];
}

export function commit(
  recipe: Dictionary,
  flow: FlowEntry[],
  triggers: Dictionary[],
  onChange: (next: Dictionary) => void,
) {
  const next = { ...recipe };
  delete next.steps;
  delete next.hitl_checkpoints;
  delete next.sinks;
  delete next.trigger;
  next.flow = flow;
  next.triggers = triggers;
  onChange(next);
}
