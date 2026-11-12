import { useCallback, useState } from "react";

export interface KanbanView {
  id: string;
  name: string;
  origin: "work" | "automations" | "all";
  query: string;
  agent: string;
  priority: string;
  attentionOnly: boolean;
}

const STORAGE_KEY = "opendesk.kanban.views.v1";

function normalize(value: unknown): KanbanView[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (!item || typeof item !== "object") return [];
    const record = item as Record<string, unknown>;
    if (typeof record.id !== "string" || typeof record.name !== "string") {
      return [];
    }
    const origin =
      record.origin === "automations" || record.origin === "all"
        ? record.origin
        : "work";
    return [
      {
        id: record.id,
        name: record.name,
        origin,
        query: typeof record.query === "string" ? record.query : "",
        agent: typeof record.agent === "string" ? record.agent : "",
        priority: typeof record.priority === "string" ? record.priority : "",
        attentionOnly: record.attentionOnly === true,
      },
    ];
  });
}

export function readKanbanViews(): KanbanView[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    return normalize(JSON.parse(raw));
  } catch {
    return [];
  }
}

export function writeKanbanViews(views: KanbanView[]): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(views));
  } catch {
    // Storage can be unavailable; views remain session-only.
  }
}

export function useKanbanViews() {
  const [views, setViews] = useState<KanbanView[]>(readKanbanViews);

  const save = useCallback((view: Omit<KanbanView, "id">) => {
    setViews((current) => {
      const id =
        typeof crypto !== "undefined" && "randomUUID" in crypto
          ? crypto.randomUUID()
          : `view-${Date.now()}`;
      const next = [...current, { ...view, id }];
      writeKanbanViews(next);
      return next;
    });
  }, []);

  const remove = useCallback((id: string) => {
    setViews((current) => {
      const next = current.filter((view) => view.id !== id);
      writeKanbanViews(next);
      return next;
    });
  }, []);

  return { views, save, remove };
}
