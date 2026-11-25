const enc = encodeURIComponent;

export type SettingsSection =
  | "general"
  | "engine"
  | "updates"
  | "notifications"
  | "queue"
  | "backups"
  | "audit"
  | "tools"
  | "about"
  | "workspaces"
  | "prompt"
  | "harness"
  | "sandbox"
  | "voice"
  | "nodes"
  | "security";

export type IntegrationSection =
  | "smtp"
  | "google"
  | "signal"
  | "telegram"
  | "channels"
  | "team"
  | "browser"
  | "inbound"
  | "sinks"
  | "vault"
  | "deliveries"
  | "hub"
  | "mcp";

export type StudioTab = "agents" | "skills" | "commands" | "mcp" | "config";

/** Canonical internal URLs. Components must not build these by hand. */
export const links = {
  dashboard: () => "/",
  kanban: (taskId?: string) =>
    taskId
      ? `/kanban?view=board&task=${enc(taskId)}`
      : "/kanban?view=board",
  queue: (jobId?: string) =>
    jobId
      ? `/kanban?view=queue&job=${enc(jobId)}`
      : "/kanban?view=queue",
  task: (id: string) => `/kanban?view=board&task=${enc(id)}`,
  taskRun: (taskId: string, runId: string) =>
    `/kanban?view=board&task=${enc(taskId)}&run=${enc(runId)}`,
  run: (id: string) => `/runs/${enc(id)}`,
  runs: (scope?: string) =>
    scope ? `/runs?scope=${enc(scope)}` : "/runs",
  chat: (threadId?: string) =>
    threadId ? `/chat?thread=${enc(threadId)}` : "/chat",
  calendar: (date?: string) =>
    date ? `/calendar?date=${enc(date)}` : "/calendar",
  calendarEvent: (id: string) => `/calendar?event=${enc(id)}`,
  artifact: (id: string) => `/artifacts?artifact=${enc(id)}`,
  routine: (scheduleId: string) => `/routines?routine=${enc(scheduleId)}`,
  /** Older name kept so existing callers keep working. */
  schedule: (scheduleId: string) => links.routine(scheduleId),
  recipe: (recipeId: string) => `/recipes?recipe=${enc(recipeId)}`,
  recipeRun: (runId: string) => `/recipe-runs/${enc(runId)}`,
  studio: (tab: StudioTab, id?: string) =>
    id
      ? `/studio?tab=${enc(tab)}&focus=${enc(id)}`
      : `/studio?tab=${enc(tab)}`,
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
  worktrees: () => "/runs?tab=worktrees",
  file: (path: string) => `/search?tab=files&path=${enc(path)}`,
  dir: (path: string) => `/search?tab=files&dir=1&path=${enc(path)}`,
  settings: (section: SettingsSection = "general") =>
    `/settings?section=${enc(section)}`,
  integration: (section: IntegrationSection = "smtp") =>
    `/integrations?section=${enc(section)}`,
  learning: (tab?: string) =>
    tab ? `/learning?tab=${enc(tab)}` : "/learning",
  governance: (section?: string) =>
    section ? `/governance?section=${enc(section)}` : "/governance",
};
