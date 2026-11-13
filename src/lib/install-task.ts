export type InstallTaskKind =
  | "skill"
  | "pack"
  | "resource"
  | "agent"
  | "command"
  | "mcp"
  | "template";

export interface InstallTaskRequest {
  kind: InstallTaskKind;
  name: string;
  source?: string | null;
  sourceType?: string | null;
  url?: string | null;
  installCommand?: string | null;
  skillId?: string | null;
  note?: string | null;
  workspaceName?: string | null;
}

export interface InstallTaskPayload {
  title: string;
  description: string;
  tags: string[];
}

const KIND_LABEL: Record<InstallTaskKind, string> = {
  skill: "skill",
  pack: "pacote",
  resource: "capacidade externa",
  agent: "agente",
  command: "comando",
  mcp: "MCP",
  template: "template de workspace",
};

/**
 * Installation of curated/external capabilities is delegated to an agent task
 * so the user can follow the actual work and result in Executions.
 */
export function buildInstallTask(request: InstallTaskRequest): InstallTaskPayload {
  const kind = KIND_LABEL[request.kind];
  const title = `Instalar ${kind} ${request.name}`;
  const lines = [
    `Instale ${kind} **${request.name}** no workspace ativo.`,
    "",
    `- Tipo: ${kind}`,
    request.source ? `- Origem: ${request.source}` : null,
    request.sourceType ? `- Formato da origem: ${request.sourceType}` : null,
    request.url ? `- Referência: ${request.url}` : null,
    request.installCommand
      ? `- Comando sugerido: \`${request.installCommand}\``
      : null,
    request.skillId ? `- Skill id: \`${request.skillId}\`` : null,
    request.workspaceName ? `- Workspace: ${request.workspaceName}` : null,
    request.note ? `- Observação: ${request.note}` : null,
    "",
    "Procedimento:",
    "1. Baixe ou execute a instalação a partir da origem acima.",
    "2. Grave os arquivos no escopo do workspace (.opencode/) ou global quando a origem exigir.",
    "3. Confira os arquivos criados e liste o resultado (caminho e tipo).",
    "4. Se a origem tiver instalador próprio, use-o e informe os passos executados.",
    "5. Se algo falhar, explique o erro e o que falta; não invente sucesso.",
    "",
    "Critério de sucesso: os arquivos da capacidade existem no destino e o resultado informa o que foi instalado.",
  ].filter((line): line is string => line !== null);

  return {
    title,
    description: lines.join("\n"),
    tags: ["instalacao", `instalacao:${request.kind}`],
  };
}
