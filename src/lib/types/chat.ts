export interface ChatThread {
  id: string;
  workspace_id: string;
  title: string;
  agent: string;
  model?: string | null;
  opencode_session_id?: string | null;
  source: string;
  external_id?: string | null;
  created_at: string;
  updated_at: string;
  last_message_at?: string | null;
}

export interface ChatMessage {
  id: string;
  thread_id: string;
  role: "user" | "assistant" | "system";
  content: string;
  metadata_json?: string | null;
  created_at: string;
}

export interface ChatReply {
  thread: ChatThread;
  user: ChatMessage;
  assistant: ChatMessage;
  usage: {
    tokens_input: number;
    tokens_output: number;
    tokens_reasoning?: number;
    tokens_cache_read?: number;
    tokens_cache_write?: number;
    estimated_cost_usd: number;
  };
  error?: string | null;
  run_id?: string | null;
  turn_id?: string | null;
}

export interface ChatPromptPreview {
  thread: ChatThread;
  prompt: string;
  sections: Array<{
    id: string;
    label: string;
    source: string;
    content: string;
  }>;
  opencode: {
    agent?: {
      id: string;
      scope: string;
      path: string;
      system: string;
    } | null;
    agents_md: Array<{ scope: string; path: string; content: string }>;
    skills: Array<{
      id: string;
      scope: string;
      name: string;
      description?: string | null;
      path: string;
      body: string;
    }>;
    commands: Array<{
      id: string;
      scope: string;
      description?: string | null;
      agent?: string | null;
      template: string;
      path: string;
    }>;
    mcp_servers: Array<{ scope: string; server: unknown }>;
    note?: string;
  };
  composed: string;
}

export interface ChatTurn {
  id: string;
  workspace_id: string;
  thread_id: string;
  run_id?: string | null;
  source: string;
  status: "RUNNING" | "SUCCESS" | "FAILED" | "CANCELLED" | string;
  agent: string;
  model?: string | null;
  tokens_input: number;
  tokens_output: number;
  tokens_reasoning: number;
  tokens_cache_read: number;
  tokens_cache_write: number;
  estimated_cost_usd: number;
  duration_ms: number;
  user_content: string;
  assistant_content?: string | null;
  error_code?: string | null;
  error_message?: string | null;
  started_at: string;
  completed_at?: string | null;
}
