export interface ScriptTool {
  id: string;
  workspace_id: string | null;
  slug: string;
  name: string;
  description: string;
  agent_prompt: string;
  runtime: string;
  custom_command: string | null;
  script_body: string;
  input_schema_json: string;
  env_secret_refs_json: string;
  timeout_secs: number;
  require_approval: boolean;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface ScriptToolList {
  tools: ScriptTool[];
  runtimes: string[];
}

export interface ScriptRunResult {
  ok: boolean;
  exit_code: number | null;
  stdout: string;
  stderr: string;
  duration_ms: number;
  timed_out: boolean;
}
