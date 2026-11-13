import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { readFavorites, useFavorites, writeFavorites } from "./favorites";

const empty = {
  packs: [],
  skills: [],
  templates: [],
  workspaces: [],
  agents: [],
  commands: [],
  mcps: [],
  resources: [],
};

describe("favorites", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("starts empty and persists every capability kind", () => {
    expect(readFavorites()).toEqual(empty);
    writeFavorites({
      ...empty,
      packs: ["essenciais"],
      skills: ["caveman"],
      templates: ["conteudo"],
      resources: ["r1"],
    });
    expect(readFavorites().templates).toEqual(["conteudo"]);
    expect(readFavorites().resources).toEqual(["r1"]);
  });

  it("migrates the v1 storage", () => {
    localStorage.setItem(
      "opendesk.favorites.v1",
      JSON.stringify({ packs: ["ok", 2], skills: ["caveman"] }),
    );
    expect(readFavorites()).toMatchObject({
      packs: ["ok"],
      skills: ["caveman"],
    });
  });

  it("ignores malformed values and unavailable storage", () => {
    localStorage.setItem("opendesk.favorites.v2", "{");
    expect(readFavorites()).toEqual(empty);
    vi.stubGlobal("localStorage", undefined);
    expect(() => writeFavorites(empty)).not.toThrow();
    expect(readFavorites()).toEqual(empty);
    vi.unstubAllGlobals();
  });
});

describe("useFavorites", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("toggles packs, skills and workspaces independently", () => {
    const { result } = renderHook(() => useFavorites());
    act(() => result.current.toggle("pack", "essenciais"));
    expect(result.current.isFavorite("pack", "essenciais")).toBe(true);
    expect(result.current.isFavorite("skill", "essenciais")).toBe(false);

    act(() => result.current.toggle("skill", "caveman"));
    act(() => result.current.toggle("template", "conteudo"));
    expect(result.current.isFavorite("template", "conteudo")).toBe(true);
    expect(readFavorites()).toMatchObject({
      packs: ["essenciais"],
      skills: ["caveman"],
      templates: ["conteudo"],
    });

    act(() => result.current.toggle("skill", "caveman"));
    expect(result.current.isFavorite("skill", "caveman")).toBe(false);
  });
});
