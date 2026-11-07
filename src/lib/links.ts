export const links = {
  task: (id: string) => `/kanban?task=${encodeURIComponent(id)}`,
  taskRun: (taskId: string, runId: string) =>
    `/kanban?task=${encodeURIComponent(taskId)}&run=${encodeURIComponent(runId)}`,
  run: (id: string) => `/runs/${encodeURIComponent(id)}`,
  runs: () => "/runs",
  artifact: (id: string) => `/artifacts?id=${encodeURIComponent(id)}`,
  schedule: (id: string) => `/routines?schedule=${encodeURIComponent(id)}`,
  agent: (id: string, scope = "workspace", workspaceId?: string | null) => {
    const params = new URLSearchParams({ kind: "agent", id, scope });
    if (workspaceId) params.set("workspace_id", workspaceId);
    return `/studio?${params.toString()}`;
  },
  file: (path: string) => `/files?path=${encodeURIComponent(path)}`,
  settings: (
    section:
      | "general"
      | "engine"
      | "updates"
      | "notifications"
      | "about"
      | "workspaces" = "general",
  ) =>
    `/settings?section=${section}`,
  integration: (
    section: "smtp" | "google" | "signal" | "deliveries" | "mcp" = "smtp",
  ) => `/integrations?section=${section}`,
};
