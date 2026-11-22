export interface SandboxPolicy {
  mode: string;
  network: boolean;
  allowed_paths: string[];
  allowed_commands: string[];
  max_runtime_minutes: number;
  scrub_env: boolean;
  image: string;
}

export interface SandboxStatus {
  policy: SandboxPolicy;
  scope: string;
  workspace_id?: string | null;
  bwrap_available: boolean;
  container_runtime?: string | null;
  platform: string;
}

export interface WindowAllowlist {
  enabled: boolean;
  windows: string[];
  apps: string[];
}

export interface Capabilities {
  available: boolean;
  backend: string;
  display?: string | null;
  screenshot_tool?: string | null;
  input_tool?: string | null;
  reason?: string | null;
}

export interface ComputerSession {
  id: string;
  status: string;
  require_approval: boolean;
  actions: number;
  screenshot_path?: string | null;
  display?: string | null;
  isolated?: boolean;
  created_at: string;
}

export interface ComputerAction {
  kind: string;
  text: string;
  key: string;
  x: string;
  y: string;
}

export const inputClass =
  "h-8 w-full rounded border border-zinc-700 bg-zinc-950 px-2 text-[12.5px] text-zinc-200";
