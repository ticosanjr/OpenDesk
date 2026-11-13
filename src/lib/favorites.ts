import { useCallback, useState } from "react";

export type FavoriteKind =
  | "pack"
  | "skill"
  | "template"
  | "workspace"
  | "agent"
  | "command"
  | "mcp"
  | "resource";

export interface FavoriteState {
  packs: string[];
  skills: string[];
  templates: string[];
  workspaces: string[];
  agents: string[];
  commands: string[];
  mcps: string[];
  resources: string[];
}

const STORAGE_KEY = "opendesk.favorites.v2";
const LEGACY_KEY = "opendesk.favorites.v1";

const FIELD: Record<FavoriteKind, keyof FavoriteState> = {
  pack: "packs",
  skill: "skills",
  template: "templates",
  workspace: "workspaces",
  agent: "agents",
  command: "commands",
  mcp: "mcps",
  resource: "resources",
};

const EMPTY: FavoriteState = {
  packs: [],
  skills: [],
  templates: [],
  workspaces: [],
  agents: [],
  commands: [],
  mcps: [],
  resources: [],
};

function normalize(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function fromRecord(record: Record<string, unknown>): FavoriteState {
  return {
    packs: normalize(record.packs),
    skills: normalize(record.skills),
    templates: normalize(record.templates),
    workspaces: normalize(record.workspaces),
    agents: normalize(record.agents),
    commands: normalize(record.commands),
    mcps: normalize(record.mcps),
    resources: normalize(record.resources),
  };
}

export function readFavorites(): FavoriteState {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed: unknown = JSON.parse(raw);
      if (parsed && typeof parsed === "object") {
        return fromRecord(parsed as Record<string, unknown>);
      }
    }
    // Migrate the v1 format (packs/skills) without losing starred items.
    const legacy = window.localStorage.getItem(LEGACY_KEY);
    if (legacy) {
      const parsed: unknown = JSON.parse(legacy);
      if (parsed && typeof parsed === "object") {
        return fromRecord(parsed as Record<string, unknown>);
      }
    }
    return EMPTY;
  } catch {
    return EMPTY;
  }
}

export function writeFavorites(state: FavoriteState): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Storage can be blocked; favorites remain session-only.
  }
}

export function useFavorites() {
  const [favorites, setFavorites] = useState<FavoriteState>(readFavorites);

  const toggle = useCallback((kind: FavoriteKind, id: string) => {
    setFavorites((current) => {
      const field = FIELD[kind];
      const list = current[field];
      const nextList = list.includes(id)
        ? list.filter((item) => item !== id)
        : [...list, id];
      const next = { ...current, [field]: nextList };
      writeFavorites(next);
      return next;
    });
  }, []);

  const isFavorite = useCallback(
    (kind: FavoriteKind, id: string) => favorites[FIELD[kind]].includes(id),
    [favorites],
  );

  return { favorites, toggle, isFavorite };
}
