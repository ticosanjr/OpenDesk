import type {
  TaskInteraction,
  TaskMessage,
  TimelineEvent,
} from "@/lib/types";

export type InteractionPermission = {
  request_id: string;
  permission_action: string;
  resources?: string | null;
  task_id?: string | null;
  run_id?: string | null;
  created_at?: string;
};

export type StreamBlock =
  | { kind: "message"; message: TaskMessage }
  | { kind: "event"; event: TimelineEvent }
  | { kind: "tools"; events: TimelineEvent[] }
  | { kind: "comments"; events: TimelineEvent[] }
  | { kind: "interaction"; interaction: TaskInteraction }
  | { kind: "permission"; permission: InteractionPermission };

export type RunEventFilter = "all" | "tools" | "errors" | "conversation";
