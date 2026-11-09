export interface PackCounts {
  agents: number;
  skills: number;
  commands: number;
  recipes: number;
  routines: number;
}

export interface PackRequires {
  tools: string[];
  integrations: string[];
}

export interface PackHealth {
  status: "installed" | "modified" | "partial" | string;
  missing_files: number;
  modified_files: number;
  missing_routines: number;
}

export interface PackSummary {
  id: string;
  version: string;
  name: string;
  category: string;
  description: string;
  icon: string;
  color: string;
  recommended: boolean;
  requires: PackRequires;
  counts: PackCounts;
  installed: boolean;
  installed_version?: string | null;
  installed_at?: string | null;
  update_available: boolean;
  health: PackHealth;
}

export interface Category {
  id: string;
  name: string;
  description: string;
}

export interface PlanFile {
  path: string;
  kind: string;
  status: "create" | "identical" | "conflict" | string;
  sha256: string;
}

export interface PlanRoutine {
  file: string;
  name: string;
  trigger_type: string;
  recipe_id?: string | null;
}

export interface InstallPlan {
  pack_id: string;
  version: string;
  files: PlanFile[];
  routines: PlanRoutine[];
  folders: string[];
  requires: PackRequires;
  conflicts: number;
}

export interface WorkspaceTemplate {
  schema: number;
  id: string;
  name: string;
  description: string;
  color: string;
  router_policy: string;
  packs: string[];
  folders: string[];
  readme?: string | null;
  memory?: string | null;
}

export interface InstallResult {
  pack_id: string;
  version: string;
  written: number;
  skipped: number;
  skipped_conflicts: number;
  routines: string[];
  folders: string[];
}

export interface PackItem {
  kind: "agent" | "skill" | "command" | "recipe" | string;
  id: string;
  name: string;
  description?: string | null;
  content: string;
  files: string[];
}

export interface ItemInstallResult {
  kind: string;
  id: string;
  written: number;
  skipped_conflicts: number;
}
