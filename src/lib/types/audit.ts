export interface AuditEntry {
  id: string;
  workspace_id?: string | null;
  action: string;
  method: string;
  path: string;
  status: number;
  duration_ms: number;
  actor: string;
  created_at: string;
}

export interface AuditResponse {
  entries: AuditEntry[];
  rate_limits: {
    read_per_minute: number;
    write_per_minute: number;
    sensitive_per_minute: number;
  };
}
