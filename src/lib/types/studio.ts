import type { DaemonStatus } from "./engine";

export type StudioScope = "workspace" | "global";

export interface PermissionRule {
  action: string;
  resource: string;
  effect: string;
}

export interface AgentFile {
  id: string;
  scope: string;
  path: string;
  name?: string | null;
  avatar?: string | null;
  description?: string | null;
  mode?: string | null;
  model?: string | null;
  color?: string | null;
  router_policy?: string | null;
  hidden: boolean;
  steps?: number | null;
  disabled: boolean;
  permissions: PermissionRule[];
  system: string;
  sha256: string;
}

export interface SkillExtraFile {
  path: string;
  content: string;
}

export interface SkillFile {
  id: string;
  scope: string;
  path: string;
  name: string;
  description?: string | null;
  body: string;
  files: SkillExtraFile[];
  sha256: string;
  disabled?: boolean;
}

export interface CommandFile {
  id: string;
  scope: string;
  path: string;
  description?: string | null;
  agent?: string | null;
  model?: string | null;
  subagent?: boolean | null;
  template: string;
  sha256: string;
  disabled?: boolean;
}

export interface ConfigDocument {
  path: string;
  content: string;
  sha256: string;
  exists: boolean;
  parsed: unknown;
}

export interface McpServerEntry {
  name: string;
  server: Record<string, unknown>;
}

export interface McpListResponse {
  document: ConfigDocument;
  servers: McpServerEntry[];
}

export const ROUTER_POLICY_LABELS: Record<string, string> = {
  COST_BENEFIT: "Melhor custo-benefício",
  CHEAPEST: "Mais barato",
  BEST_QUALITY: "Máxima qualidade",
  MANUAL: "Manual",
  CUSTOM_RULES: "Regras customizadas",
};

export type StudioKind = "agent" | "skill" | "command" | "mcp";

export interface EffectiveStudioItem {
  kind: StudioKind;
  id: string;
  source_scope: "global" | "workspace" | "ancestor" | "runtime";
  source_path?: string | null;
  overridden: boolean;
  disabled: boolean;
  value: Record<string, unknown>;
}

export interface StudioInstallItem {
  kind: StudioKind;
  id: string;
  name?: string | null;
  description?: string | null;
  body: string;
  files?: SkillExtraFile[];
  server?: Record<string, unknown> | null;
}

export interface StudioInstallPreview {
  items: StudioInstallItem[];
  warnings: string[];
}

export interface StudioCatalog {
  repos?: Array<{
    id: string;
    name: string;
    description: string;
    source_type: "github" | "npm" | "url" | "local";
    source: string;
    kind: "agent" | "skill" | "command" | "mcp";
  }>;
  mcps: Array<{
    id: string;
    name: string;
    description: string;
    server: Record<string, unknown>;
  }>;
}

export interface BackupEntry {
  id: string;
  original_path: string;
  file_name: string;
  created_at: string;
  sha256?: string | null;
}

export interface StudioRuntime {
  engine: DaemonStatus;
  agents: Array<{
    id: string;
    name?: string | null;
    description?: string | null;
    mode?: string | null;
    model?: { providerID: string; id: string; variant?: string | null } | null;
  }>;
  models: Array<{
    id: string;
    modelID?: string | null;
    providerID?: string | null;
    name?: string | null;
    status?: string | null;
    enabled?: boolean | null;
  }>;
  skills: Array<{
    id: string;
    name?: string | null;
    description?: string | null;
    path?: string | null;
    content?: string | null;
  }>;
  commands: Array<{
    name: string;
    description?: string | null;
  }>;
  mcps: Array<{
    name: string;
    status?: { status?: string | null } | string | null;
  }>;
}
