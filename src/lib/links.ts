export const links = {
  task: (id: string) => `/kanban?task=${encodeURIComponent(id)}`,
  taskRun: (taskId: string, runId: string) =>
    `/kanban?task=${encodeURIComponent(taskId)}&run=${encodeURIComponent(runId)}`,
  run: (id: string) => `/runs/${encodeURIComponent(id)}`,
  runs: () => "/runs",
  calendar: (date?: string) =>
    date ? `/calendar?date=${encodeURIComponent(date)}` : "/calendar",
  calendarEvent: (id: string) =>
    `/calendar?event=${encodeURIComponent(id)}`,
  artifact: (id: string) => `/artifacts?id=${encodeURIComponent(id)}`,
  schedule: (id: string) => `/routines?schedule=${encodeURIComponent(id)}`,
  agent: (id: string, scope = "workspace", workspaceId?: string | null) => {
    const params = new URLSearchParams({ kind: "agent", id, scope });
    if (workspaceId) params.set("workspace_id", workspaceId);
    return `/studio?${params.toString()}`;
  },
  skill: (id: string, scope = "workspace", workspaceId?: string | null) => {
    const params = new URLSearchParams({ kind: "skill", id, scope });
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
      | "tools"
      | "about"
      | "workspaces" = "general",
  ) =>
    `/settings?section=${section}`,
  integration: (
    section:
      | "smtp"
      | "google"
      | "signal"
      | "inbound"
      | "sinks"
      | "deliveries"
      | "mcp" = "smtp",
  ) => `/integrations?section=${section}`,
};
