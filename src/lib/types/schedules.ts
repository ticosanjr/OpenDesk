export interface Schedule {
  id: string;
  workspace_id: string;
  name: string;
  description?: string | null;
  cron_expression: string;
  timezone: string;
  agent_name?: string | null;
  command_or_prompt: string;
  trigger_type: string;
  trigger_config_json: string;
  target_kind: string;
  recipe_id?: string | null;
  router_policy: string;
  output_folder_template: string;
  notify_email_recipients?: string | null;
  notify_signal: boolean;
  notify_signal_recipients?: string | null;
  notify_telegram: boolean;
  notify_telegram_recipients?: string | null;
  notify_only_on_change: boolean;
  recurrence_json?: string | null;
  one_time: boolean;
  last_output_hash?: string | null;
  misfire_policy: string;
  safe_readonly_mode: boolean;
  sync_to_google_docs: boolean;
  sync_to_google_calendar: boolean;
  is_active: boolean;
  last_run_at?: string | null;
  last_run_status?: string | null;
  next_run_at?: string | null;
  created_at: string;
}

export interface IntegrationOverviewItem {
  id: string;
  label: string;
  configured: boolean;
  active: boolean;
  detail?: string | null;
  href: string;
}

export interface SmtpSettings {
  workspace_id: string;
  host?: string | null;
  port: number;
  username?: string | null;
  from_email?: string | null;
  use_tls: boolean;
  password_set: boolean;
  configured: boolean;
}

export interface Delivery {
  id: string;
  workspace_id: string;
  artifact_id?: string | null;
  channel: string;
  idempotency_key: string;
  status: string;
  attempts: number;
  last_error?: string | null;
  external_id?: string | null;
  external_url?: string | null;
  next_retry_at?: string | null;
  max_attempts?: number;
  created_at: string;
  updated_at: string;
}

export interface GoogleStatus {
  connected: boolean;
  email?: string | null;
  scopes?: string | null;
  callback_url: string;
  client_id_set: boolean;
  client_secret_set: boolean;
}

export interface GoogleConfig {
  client_id_set: boolean;
  client_secret_set: boolean;
  scopes: string;
  callback_url: string;
}

export interface SignalConfig {
  enabled: boolean;
  account: string;
  cli_path: string;
  allowlist: string[];
  workspace_id?: string | null;
  agent: string;
}

export interface SignalStatus {
  config: SignalConfig;
  cli_available: boolean;
  cli_version?: string | null;
  bridge_running: boolean;
  link_running: boolean;
}

export interface TelegramConfig {
  enabled: boolean;
  allowlist: string[];
  workspace_id?: string | null;
  agent: string;
}

export interface TelegramStatus {
  config: TelegramConfig;
  token_configured: boolean;
  bot_username?: string | null;
  bot_name?: string | null;
  bridge_running: boolean;
  last_error?: string | null;
}

export interface Recurrence {
  frequency:
    | "once"
    | "hourly"
    | "daily"
    | "weekly"
    | "weekdays"
    | "monthly"
    | "custom";
  interval: number;
  time: string;
  weekdays: number[];
  day_of_month?: number | null;
  start_date?: string | null;
  end_date?: string | null;
  cron_expression?: string | null;
}

export interface SchedulePreview {
  cron_expression: string;
  human: string;
  next_runs: string[];
}

export type ChannelBotStatus = "stopped" | "running" | "error";
export type ChannelChatType = "group" | "private";
export type ChannelMemoryScope = "chat" | "normal" | "both" | "none";

export interface ChannelBot {
  id: string;
  workspace_id: string;
  channel: string;
  name: string;
  enabled: boolean;
  token_configured: boolean;
  allowlist: string[];
  default_agent: string;
  status: ChannelBotStatus | string;
  last_error?: string | null;
  last_checked_at?: string | null;
  running: boolean;
  config_json: string;
  created_at: string;
  updated_at: string;
}

export interface ChannelChatContext {
  id: string;
  bot_id: string;
  workspace_id: string;
  channel: string;
  external_chat_id: string;
  chat_type: ChannelChatType | string;
  title: string;
  system_context: string;
  agent_override?: string | null;
  memory_scope: ChannelMemoryScope;
  opencode_session_id?: string | null;
  enabled: boolean;
  last_message_at?: string | null;
  created_at: string;
  updated_at: string;
}
