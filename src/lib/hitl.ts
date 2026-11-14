export interface HitlView {
  interaction_type: string;
  on_timeout?: string | null;
  options_json?: string | null;
  context_json?: string | null;
  expires_at?: string | null;
}

export interface HitlConsequences {
  approve: string;
  reject: string;
  timeout: string;
}

export const HITL_TYPE_LABEL: Record<string, string> = {
  APPROVAL: "Aprovação",
  BINARY_GATE: "Aprovação binária",
  APPROVAL_WITH_FEEDBACK: "Aprovação com ajustes",
  ARTIFACT_EDIT_HANDOFF: "Aprovação de artefato",
  TOOL_PERMISSION_PROXY: "Permissão de ferramenta",
  SINGLE_CHOICE: "Escolha de opção",
  MULTI_CHOICE: "Múltipla escolha",
  TEXT: "Resposta livre",
  ACKNOWLEDGE: "Ciência",
  ARTIFACT_REVIEW: "Revisão de artefato",
};

export function hitlTypeLabel(value: string): string {
  return HITL_TYPE_LABEL[value] ?? value;
}

export function parseHitlOptions(value?: string | null): string[] {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : [];
  } catch {
    return [];
  }
}

export function parseHitlContext(value?: string | null): Record<string, unknown> {
  if (!value) return {};
  try {
    const parsed: unknown = JSON.parse(value);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

export function hitlConsequences(view: HitlView): HitlConsequences {
  const context = parseHitlContext(view.context_json);
  const next =
    typeof context.next_step === "string" && context.next_step.trim()
      ? context.next_step.trim()
      : "a próxima etapa";
  const timeout = (view.on_timeout ?? "").toUpperCase();
  return {
    approve:
      view.interaction_type === "ACKNOWLEDGE"
        ? "Confirma a ciência e retoma o fluxo."
        : `Retoma o fluxo e segue para ${next}.`,
    reject:
      view.interaction_type === "ACKNOWLEDGE"
        ? "Apenas registra a ciência; não interrompe o fluxo."
        : "Encerra este run, mantém os artefatos já gerados e não executa as etapas seguintes.",
    timeout:
      timeout === "AUTO_APPROVE"
        ? "Se ninguém responder, o fluxo aprova automaticamente ao expirar."
        : timeout === "AUTO_REJECT"
          ? "Se ninguém responder, o fluxo é rejeitado ao expirar."
          : "Se ninguém responder, o fluxo pausa e gera um alerta.",
  };
}

export function hitlSecondsLeft(expiresAt?: string | null, now = Date.now()): number {
  if (!expiresAt) return 0;
  const time = Date.parse(expiresAt);
  if (!Number.isFinite(time)) return 0;
  return Math.max(0, Math.round((time - now) / 1000));
}
