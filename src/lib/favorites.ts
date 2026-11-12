import { useCallback, useState } from "react";

export type FavoriteKind = "pack" | "skill";

export interface FavoriteState {
  packs: string[];
  skills: string[];
}

const STORAGE_KEY = "opendesk.favorites.v1";
const EMPTY: FavoriteState = { packs: [], skills: [] };

function normalize(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

export function readFavorites(): FavoriteState {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return EMPTY;
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return EMPTY;
    const record = parsed as Record<string, unknown>;
    return {
      packs: normalize(record.packs),
      skills: normalize(record.skills),
    };
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
      const list = current[kind === "pack" ? "packs" : "skills"];
      const nextList = list.includes(id)
        ? list.filter((item) => item !== id)
        : [...list, id];
      const next = {
        ...current,
        [kind === "pack" ? "packs" : "skills"]: nextList,
      } as FavoriteState;
      writeFavorites(next);
      return next;
    });
  }, []);

  const isFavorite = useCallback(
    (kind: FavoriteKind, id: string) =>
      (kind === "pack" ? favorites.packs : favorites.skills).includes(id),
    [favorites],
  );

  return { favorites, toggle, isFavorite };
}
