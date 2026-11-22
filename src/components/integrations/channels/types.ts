export interface ChannelStatus {
  channel: string;
  routes: number;
  webhook_token_set: boolean;
  webhook_path: string;
  connector: string;
}

export interface ChannelRoute {
  id: string;
  channel: string;
  identity: string;
  agent: string;
  permission_profile: string;
  trust_level: string;
  created_at: string;
}

export interface WhatsAppConfig {
  enabled: boolean;
  provider: string;
  phone_number: string;
  phone_number_id: string;
  business_account_id: string;
  api_version: string;
  workspace_id?: string | null;
  notes: string;
}

export const CHANNELS = ["whatsapp", "telegram", "signal", "email", "web", "api"];
export const TRUST = ["untrusted", "internal", "trusted"];
export const PROFILES = ["read_only", "default", "full"];

export const inputClass =
  "h-8 rounded border border-zinc-700 bg-zinc-950 px-2 text-[12.5px] text-zinc-200";
