export const RECENT_SEARCH_KEY = "opendesk.search.recent";
export const MAX_RECENT_SEARCHES = 6;

type ReadableStorage = Pick<Storage, "getItem">;
type WritableStorage = Pick<Storage, "setItem">;

function clean(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const output: string[] = [];
  for (const item of value) {
    if (typeof item !== "string") continue;
    const query = item.trim();
    if (query.length < 2 || seen.has(query)) continue;
    seen.add(query);
    output.push(query);
    if (output.length >= MAX_RECENT_SEARCHES) break;
  }
  return output;
}

export function loadRecentSearches(storage?: ReadableStorage | null): string[] {
  if (!storage) return [];
  try {
    return clean(JSON.parse(storage.getItem(RECENT_SEARCH_KEY) ?? "[]"));
  } catch {
    return [];
  }
}

export function pushRecentSearch(list: string[], query: string): string[] {
  const value = query.trim();
  if (value.length < 2) return list;
  return [value, ...list.filter((item) => item !== value)].slice(
    0,
    MAX_RECENT_SEARCHES,
  );
}

export function saveRecentSearches(
  storage: WritableStorage | null | undefined,
  list: string[],
): void {
  if (!storage) return;
  try {
    storage.setItem(RECENT_SEARCH_KEY, JSON.stringify(clean(list)));
  } catch {
    // Storage can be unavailable (private mode); recent searches are optional.
  }
}
