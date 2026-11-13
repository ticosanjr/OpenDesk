import { Search, Star } from "lucide-react";

export type PacksTab =
  | "all"
  | "workspaces"
  | "packs"
  | "resources"
  | "recommended";

export function PacksToolbar({
  tab,
  query,
  counts,
  searching,
  filteredTemplates,
  filteredPacks,
  favoritesOnly = false,
  onTabChange,
  onQueryChange,
  onToggleFavorites,
}: {
  tab: PacksTab;
  query: string;
  counts: { templates: number; packs: number };
  searching: boolean;
  filteredTemplates: number;
  filteredPacks: number;
  favoritesOnly?: boolean;
  onTabChange: (tab: PacksTab) => void;
  onQueryChange: (value: string) => void;
  onToggleFavorites: () => void;
}) {
  const tabs: Array<{ id: PacksTab; label: string }> = [
    { id: "all", label: "Visão geral" },
    { id: "workspaces", label: `Workspaces (${counts.templates})` },
    { id: "packs", label: `Pacotes (${counts.packs})` },
    { id: "resources", label: "Capacidades externas" },
    { id: "recommended", label: "Recomendados" },
  ];

  return (
    <div className="flex flex-col gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 p-2">
      <div className="flex flex-wrap items-center gap-1">
        {tabs.map((item) => (
          <button
            key={item.id}
            type="button"
            aria-pressed={tab === item.id}
            onClick={() => onTabChange(item.id)}
            className={
              tab === item.id
                ? "inline-flex h-8 items-center rounded-lg bg-zinc-800 px-3 text-[13.5px] font-medium text-zinc-100"
                : "inline-flex h-8 items-center rounded-lg px-3 text-[13.5px] font-medium text-zinc-500 transition-colors hover:bg-zinc-900 hover:text-zinc-200"
            }
          >
            {item.label}
          </button>
        ))}
        <button
          type="button"
          aria-pressed={favoritesOnly}
          onClick={onToggleFavorites}
          className={
            favoritesOnly
              ? "inline-flex h-8 items-center gap-1.5 rounded-lg border border-amber-500/40 bg-amber-500/10 px-2.5 text-[13px] text-amber-200"
              : "inline-flex h-8 items-center gap-1.5 rounded-lg border border-zinc-800 px-2.5 text-[13px] text-zinc-500 transition-colors hover:border-zinc-700 hover:text-zinc-200"
          }
        >
          <Star size={13} fill={favoritesOnly ? "currentColor" : "none"} />
          Só favoritos
        </button>
        <div className="ml-auto flex h-9 min-w-[240px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5 focus-within:border-zinc-600 sm:max-w-md">
          <Search size={13} className="shrink-0 text-zinc-500" />
          <input
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="Buscar nos workspaces, pacotes e capacidades externas…"
            className="h-full min-w-0 flex-1 bg-transparent text-[14px] outline-none placeholder:text-zinc-500"
            aria-label="Buscar workspaces, pacotes e capacidades externas"
          />
        </div>
      </div>
      {searching ? (
        <p className="text-[11.5px] text-zinc-500">
          Resultados em {filteredTemplates} workspace(s), {filteredPacks} pacote(s),
          capacidades externas e recomendações.
        </p>
      ) : null}
    </div>
  );
}
