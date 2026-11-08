export interface CalendarEvent {
  id: string;
  workspace_id: string;
  source_id?: string | null;
  external_id?: string | null;
  title: string;
  description?: string | null;
  location?: string | null;
  start_at: string;
  end_at: string;
  all_day: boolean;
  timezone: string;
  status: "CONFIRMED" | "TENTATIVE" | "CANCELLED" | string;
  color_hex?: string | null;
  recurrence_rule?: string | null;
  created_at: string;
  updated_at: string;
}

export interface CalendarSource {
  id: string;
  workspace_id: string;
  provider: string;
  external_id?: string | null;
  name: string;
  color_hex: string;
  timezone: string;
  is_visible: boolean;
  is_read_only: boolean;
  sync_enabled: boolean;
  sync_token?: string | null;
  last_synced_at?: string | null;
  created_at: string;
}

export interface CalendarItem {
  id: string;
  kind: "event" | "routine" | "recipe" | "run" | "recipe_run" | "queued" | string;
  title: string;
  subtitle?: string | null;
  start: string;
  end?: string | null;
  all_day: boolean;
  status: "PAST" | "ACTIVE" | "FUTURE" | string;
  source: string;
  color?: string | null;
  href?: string | null;
  editable: boolean;
}

export interface CalendarAgenda {
  from: string;
  to: string;
  items: CalendarItem[];
}
