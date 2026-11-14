export interface DaemonStatus {
  online: boolean;
  port: number;
  active_jobs: number;
  state: string;
  message?: string | null;
  version?: string | null;
  mode?: string;
  binary_path?: string | null;
}

export interface OpenCodeBinaryInfo {
  path: string;
  version: string;
  method?: string | null;
  candidates: string[];
}

export interface EngineStatus {
  daemon: DaemonStatus;
  configured_binary?: string | null;
  resolved?: OpenCodeBinaryInfo | null;
  resolve_error?: string | null;
  candidates: string[];
  app_version: string;
}

export interface DetectResult {
  ok: boolean;
  binary?: OpenCodeBinaryInfo | null;
  error?: string | null;
  candidates: string[];
}

export interface OpenCodeRelease {
  version: string;
  tag: string;
  name: string;
  notes?: string | null;
  url: string;
  published_at?: string | null;
}

export interface OpenCodeUpdateCheck {
  current?: string | null;
  latest?: string | null;
  update_available: boolean;
  release?: OpenCodeRelease | null;
  checked_at: string;
  error?: string | null;
}

export interface EngineUpdateResult {
  ok: boolean;
  version?: string | null;
  output: string;
  daemon: DaemonStatus;
}

export interface UpdateAsset {
  name: string;
  size: number;
  sha256: string;
}

export interface LatestManifest {
  version: string;
  tag: string;
  repo: string;
  platform: string;
  published_at: string;
  notes: string;
  assets: UpdateAsset[];
}

export interface UpdateInfo {
  current: string;
  latest?: LatestManifest | null;
  update_available: boolean;
  token_configured: boolean;
  installer: string;
  checked_at: string;
  error?: string | null;
}

export interface DownloadResult {
  ok: boolean;
  action: string;
  path?: string | null;
  message: string;
}

export interface ModelInfo {
  id: string;
  modelID?: string | null;
  providerID?: string | null;
  name?: string | null;
  variants?: { id: string }[];
  status?: string | null;
  enabled?: boolean | null;
}

export interface AgentInfo {
  id: string;
  name?: string | null;
  description?: string | null;
  mode?: string | null;
  model?: { providerID: string; id: string; variant?: string | null } | null;
  color?: string | null;
  hidden?: boolean;
  permissions?: unknown;
}

export interface AboutInfo {
  app: {
    name: string;
    version: string;
    phase: string;
    api: string;
  };
  runtime: {
    tauri: string;
    rustc?: string | null;
    os: string;
    arch: string;
    kernel?: string | null;
  };
  engine: {
    daemon: DaemonStatus;
    configured_binary?: string | null;
    resolved?: OpenCodeBinaryInfo | null;
    installer: string;
  };
  database: {
    path: string;
    size_bytes: number;
    schema_version?: number | null;
    workspaces: number;
  };
  paths: {
    data_dir: string;
    config_dir: string;
    log_dir: string;
    db_path: string;
    backups_dir: string;
    run_logs_dir: string;
  };
  updates: {
    current: string;
    installer: string;
    token_configured: boolean;
  };
  desktop: DesktopIntegration;
  logs: Array<{
    name?: string | null;
    path: string;
    size_bytes: number;
    modified?: string | null;
  }>;
  started_at: string;
  api_port: number;
}

export interface DesktopIntegration {
  supported: boolean;
  appimage?: string | null;
  desktop_file?: string | null;
  icon_path?: string | null;
  installed: boolean;
  message?: string | null;
}

export interface ModelHealth {
  provider_id: string;
  model_id: string;
  code: string;
  failures: number;
  failed_at: string;
}
