import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { StudioCatalog } from "@/lib/types";

type CatalogRepo = NonNullable<StudioCatalog["repos"]>[number];
type CatalogMcp = StudioCatalog["mcps"][number];

export function InstallCatalogPane({
  catalog,
  onPickRepo,
  onSelectPreset,
}: {
  catalog: StudioCatalog | null;
  onPickRepo: (repo: CatalogRepo) => void;
  onSelectPreset: (preset: CatalogMcp) => void;
}) {
  return (
    <div className="space-y-3">
      {catalog?.repos?.length ? (
        <div className="space-y-1.5">
          <p className="text-[13px] font-semibold uppercase tracking-wide text-zinc-500">
            Repositórios recomendados
          </p>
          {catalog.repos.map((repo) => (
            <button
              key={repo.id}
              type="button"
              onClick={() => onPickRepo(repo)}
              className="flex w-full items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-left transition-colors hover:border-blue-500/30 hover:bg-zinc-900/50"
              title={`${repo.source_type}: ${repo.source}`}
            >
              <div className="min-w-0 flex-1">
                <p className="text-[14px] font-medium text-zinc-200">
                  {repo.name}
                </p>
                <p className="truncate text-[11.5px] text-zinc-500">
                  {repo.description}
                </p>
              </div>
              <Badge variant="blue">{repo.kind}</Badge>
              <span className="font-mono text-[12px] text-zinc-500">
                {repo.source}
              </span>
            </button>
          ))}
        </div>
      ) : null}

      <p className="text-[13px] font-semibold uppercase tracking-wide text-zinc-500">
        Presets MCP
      </p>
      {(catalog?.mcps ?? []).map((preset) => (
        <div
          key={preset.id}
          className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
        >
          <div className="min-w-0 flex-1">
            <p className="text-[13px] text-zinc-200">{preset.name}</p>
            <p className="truncate text-[11.5px] text-zinc-500">{preset.description}</p>
          </div>
          <Button
            size="sm"
            variant="outline"
            onClick={() => onSelectPreset(preset)}
          >
            selecionar
          </Button>
        </div>
      ))}
    </div>
  );
}
