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
}
