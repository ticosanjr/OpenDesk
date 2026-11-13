import { describe, expect, it } from "vitest";
import {
  MAX_RECENT_SEARCHES,
  RECENT_SEARCH_KEY,
  loadRecentSearches,
  pushRecentSearch,
  saveRecentSearches,
} from "./recent-searches";

function storage(initial?: string) {
  const data = new Map<string, string>();
  if (initial !== undefined) data.set(RECENT_SEARCH_KEY, initial);
  return {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
    dump: () => data.get(RECENT_SEARCH_KEY) ?? null,
  };
}

describe("recent searches", () => {
  it("deduplicates, trims and caps the list", () => {
    let list = pushRecentSearch([], "  relatório ");
    list = pushRecentSearch(list, "relatório");
    list = pushRecentSearch(list, "a");
    for (let index = 0; index < 10; index += 1) {
      list = pushRecentSearch(list, `busca ${index}`);
    }
    expect(list).toHaveLength(MAX_RECENT_SEARCHES);
    expect(list[0]).toBe("busca 9");
    expect(list.filter((item) => item === "busca 9")).toHaveLength(1);
    expect(list).not.toContain("relatório");
  });

  it("round-trips only valid entries", () => {
    const store = storage();
    saveRecentSearches(store, ["relatório", "relatório", "", "x", "kanban"]);
    expect(loadRecentSearches(store)).toEqual(["relatório", "kanban"]);
  });

  it("survives corrupt or missing storage", () => {
    expect(loadRecentSearches(storage("{not json"))).toEqual([]);
    expect(loadRecentSearches(null)).toEqual([]);
    saveRecentSearches(null, ["ok"]);
    saveRecentSearches(storage(), ["ok"]);
  });
});
