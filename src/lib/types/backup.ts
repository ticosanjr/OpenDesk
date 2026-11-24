export interface BackupIncludes {
  workspace_files: boolean;
  sqlite_state: boolean;
  run_logs: boolean;
  encrypted_secrets: boolean;
}

export interface WorkspaceBackup {
  id: string;
  workspace_id: string;
  workspace_name: string;
  file_name: string;
  sha256: string;
  size_bytes: number;
  files: number;
  bytes: number;
  created_at: string;
  version: number;
  includes?: BackupIncludes;
  counts?: Record<string, number>;
}

export interface BackupOptions {
  include_logs?: boolean;
  passphrase?: string;
}

export type RestoreMode = "new_workspace" | "in_place";

export interface RestoreReport {
  workspace_id: string;
  mode: RestoreMode;
  counts: Record<string, number>;
  missing_secrets: string[];
  pre_restore_backup?: string | null;
}
