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
    estimated_cost_usd: number;
  };
  error?: string | null;
}
