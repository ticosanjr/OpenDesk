export type SortKey = "recent" | "duration" | "cost" | "tokens";
export type RunsView = "cards" | "table";

export const STATUSES: Array<[string, string]> = [
  ["", "todos os status"],
  ["SUCCESS", "sucesso"],
  ["FAILED", "falhou"],
  ["RUNNING", "executando"],
  ["CANCELLED", "cancelada"],
];

export const SORTS: Array<[SortKey, string]> = [
  ["recent", "mais recentes"],
  ["duration", "maior duração"],
  ["cost", "maior custo"],
  ["tokens", "mais tokens"],
];
