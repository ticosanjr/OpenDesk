import { Boxes, CheckSquare, RefreshCw, Star, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { cn } from "@/lib/utils";
import type { Category, PackSummary } from "./pack-types";
import { PackCard } from "./PackCard";

export type PackInstallFilter = "all" | "installed" | "available" | "favorites";

export function PackCatalog({
  packs,
  filtered,
  categories,
  category,
  installFilter,
  installedCount,
  loading,
  isFavorite,
  managing = false,
  selectedPacks = new Set<string>(),
  updatableCount = 0,
  updating = false,
  onCategoryChange,
  onInstallFilterChange,
  onOpenPack,
  onToggleFavorite,
  onToggleManage,
  onTogglePack,
  onSelectInstalled,
  onClearSelection,
  onUpdateAll,
  onRemoveSelected,
  onClearFilters,
}: {
  packs: PackSummary[];
  filtered: PackSummary[];
  categories: Category[];
  category: string;
  installFilter: PackInstallFilter;
  installedCount: number;
  loading: boolean;
  isFavorite: (id: string) => boolean;
  managing?: boolean;
  selectedPacks?: Set<string>;
  updatableCount?: number;
  updating?: boolean;
  onCategoryChange: (value: string) => void;
  onInstallFilterChange: (value: PackInstallFilter) => void;
  onOpenPack: (pack: PackSummary) => void;
  onToggleFavorite: (id: string) => void;
  onToggleManage: () => void;
  onTogglePack: (id: string) => void;
  onSelectInstalled: () => void;
  onClearSelection: () => void;
  onUpdateAll: () => void;
  onRemoveSelected: () => void;
  onClearFilters: () => void;
}) {
  return (
    <>
      <div className="flex flex-wrap items-end justify-between gap-3 pt-1">
        <div>
          <h2 className="text-[14px] font-semibold text-zinc-100">
            Adicionar capacidades
          </h2>
          <p className="mt-0.5 text-[12px] text-zinc-500">
            Instale pacotes no workspace ativo ou use um workspace pronto como base.
          </p>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-3 py-2">
        <select
          value={category}
          onChange={(event) => onCategoryChange(event.target.value)}
          className="h-8 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2 text-[12px] text-zinc-300"
          aria-label="Filtrar por categoria"
        >
          <option value="all">todas as categorias</option>
          {categories.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
        <div className="flex rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
          {(
            [
              ["all", "Todos"],
              ["installed", `Instalados (${installedCount})`],
              ["available", "Disponíveis"],
              ["favorites", "Favoritos"],
            ] as const
          ).map(([value, label]) => (
            <button
              key={value}
              type="button"
              aria-pressed={installFilter === value}
              onClick={() => onInstallFilterChange(value)}
              className={cn(
                "inline-flex h-8 items-center rounded-md px-2.5 text-[13.5px] transition-colors",
                installFilter === value
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:text-zinc-200",
              )}
            >
              {label}
            </button>
          ))}
        </div>
        <span className="text-[12px] text-zinc-500">
          {filtered.length} de {packs.length}
        </span>
        <div className="ml-auto flex flex-wrap items-center gap-1.5">
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-[12px]"
            onClick={onUpdateAll}
            disabled={updating || updatableCount === 0}
            title="Reinstala os pacotes com atualização disponível preservando arquivos modificados"
          >
            <RefreshCw size={12} className={updating ? "animate-spin" : undefined} />
            Atualizar todos
            {updatableCount > 0 ? ` (${updatableCount})` : ""}
          </Button>
          <Button
            variant={managing ? "default" : "outline"}
            size="sm"
            className="h-7 text-[12px]"
            onClick={onToggleManage}
          >
            <CheckSquare size={12} />
            {managing ? "Concluir" : "Gerenciar"}
          </Button>
        </div>
      </div>

      {managing ? (
        <div className="flex flex-wrap items-center gap-1.5 rounded-lg border border-blue-500/25 bg-blue-500/[0.05] px-3 py-2">
          <span className="text-[12.5px] text-blue-100">
            {selectedPacks.size} selecionado(s)
          </span>
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-[12px]"
            onClick={onSelectInstalled}
            disabled={installedCount === 0}
          >
            Selecionar instalados
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-[12px]"
            onClick={onClearSelection}
            disabled={selectedPacks.size === 0}
          >
            limpar
          </Button>
          <Button
            variant="destructive"
            size="sm"
            className="ml-auto h-7 text-[12px]"
            onClick={onRemoveSelected}
            disabled={selectedPacks.size === 0}
          >
            <Trash2 size={12} />
            Remover selecionados
          </Button>
        </div>
      ) : null}

      {loading ? (
        <LoadingState label="Carregando pacotes…" rows={6} />
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={installFilter === "favorites" ? Star : Boxes}
          title={
            installFilter === "favorites"
              ? "Nenhum pacote favorito"
              : "Nenhum pacote encontrado"
          }
          description={
            installFilter === "favorites"
              ? "Toque na estrela de um pacote para favoritar e encontrá-lo aqui."
              : "Ajuste o filtro de categoria ou limpe a busca."
          }
          compact
          action={
            installFilter === "favorites" ? (
              <Button size="sm" variant="outline" onClick={onClearFilters}>
                Ver todos os pacotes
              </Button>
            ) : undefined
          }
        />
      ) : (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {filtered.map((pack) => (
            <PackCard
              key={pack.id}
              pack={pack}
              favorite={isFavorite(pack.id)}
              selectable={managing && pack.installed}
              selected={selectedPacks.has(pack.id)}
              onOpen={() => onOpenPack(pack)}
              onToggleFavorite={() => onToggleFavorite(pack.id)}
              onSelect={() => onTogglePack(pack.id)}
            />
          ))}
        </div>
      )}
    </>
  );
}
