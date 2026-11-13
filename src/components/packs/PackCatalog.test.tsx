import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PackCatalog } from "./PackCatalog";
import type { PackSummary } from "./pack-types";

const pack = {
  id: "controle",
  name: "Controle",
  description: "Pacote base",
  category: "base",
  color: "#3b82f6",
  installed: true,
  recommended: false,
  update_available: true,
  counts: { agents: 1, recipes: 0, skills: 1, routines: 0 },
  requires: { tools: [], integrations: [] },
  health: { status: "ok" },
} as unknown as PackSummary;

function renderCatalog(overrides: Partial<Parameters<typeof PackCatalog>[0]> = {}) {
  return render(
    <PackCatalog
      packs={[pack]}
      filtered={[pack]}
      categories={[]}
      category="all"
      installFilter="all"
      installedCount={1}
      loading={false}
      isFavorite={() => false}
      onCategoryChange={vi.fn()}
      onInstallFilterChange={vi.fn()}
      onOpenPack={vi.fn()}
      onToggleFavorite={vi.fn()}
      onToggleManage={vi.fn()}
      onTogglePack={vi.fn()}
      onSelectInstalled={vi.fn()}
      onClearSelection={vi.fn()}
      onUpdateAll={vi.fn()}
      onRemoveSelected={vi.fn()}
      onClearFilters={vi.fn()}
      {...overrides}
    />,
  );
}

describe("PackCatalog management", () => {
  it("offers update all with the pending count", () => {
    const onUpdateAll = vi.fn();
    renderCatalog({ updatableCount: 1, onUpdateAll });
    const button = screen.getByRole("button", { name: /Atualizar todos \(1\)/ });
    fireEvent.click(button);
    expect(onUpdateAll).toHaveBeenCalled();
  });

  it("explains an empty favorites view and clears the filter", () => {
    const onClearFilters = vi.fn();
    renderCatalog({
      filtered: [],
      installFilter: "favorites",
      onClearFilters,
    });
    expect(screen.getByText("Nenhum pacote favorito")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Ver todos os pacotes" }));
    expect(onClearFilters).toHaveBeenCalled();
  });

  it("selects installed packs in manage mode", () => {
    const onTogglePack = vi.fn();
    renderCatalog({ managing: true, onTogglePack });
    fireEvent.click(screen.getByLabelText("Selecionar Controle"));
    expect(onTogglePack).toHaveBeenCalledWith("controle");
  });
});
