import { beforeEach, describe, expect, it, vi } from "vitest";
import { readFavorites, writeFavorites } from "./favorites";

describe("favorites", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("starts empty and persists packs and skills", () => {
    expect(readFavorites()).toEqual({ packs: [], skills: [] });
    writeFavorites({ packs: ["essenciais"], skills: ["caveman"] });
    expect(readFavorites()).toEqual({
      packs: ["essenciais"],
      skills: ["caveman"],
    });
  });

  it("ignores malformed values", () => {
    localStorage.setItem("opendesk.favorites.v1", "{");
    expect(readFavorites()).toEqual({ packs: [], skills: [] });
    localStorage.setItem(
      "opendesk.favorites.v1",
      JSON.stringify({ packs: ["ok", 2], skills: "nope" }),
    );
    expect(readFavorites()).toEqual({ packs: ["ok"], skills: [] });
  });

  it("survives unavailable storage", () => {
    vi.stubGlobal("localStorage", undefined);
    expect(() => writeFavorites({ packs: ["x"], skills: [] })).not.toThrow();
    expect(readFavorites()).toEqual({ packs: [], skills: [] });
    vi.unstubAllGlobals();
  });
});
