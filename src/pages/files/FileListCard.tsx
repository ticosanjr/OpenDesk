import {
  ChevronRight,
  FileText,
  Folder,
  FolderOpen,
  Home,
} from "lucide-react";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { FilterSearch } from "@/components/ui/filter-bar";
import { LoadingState } from "@/components/ui/loading-state";
import type { FileEntry } from "@/lib/types";
import { formatNumber, relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { Crumb, FileScope } from "./types";

export function FileListCard({
  crumbs,
  filter,
  onFilterChange,
  scope,
  onScopeChange,
  onLoadTree,
  loading,
  searching,
  filtered,
  selected,
  onEntryClick,
}: {
  crumbs: Crumb[];
  filter: string;
  onFilterChange: (value: string) => void;
  scope: FileScope;
  onScopeChange: (scope: FileScope) => void;
  onLoadTree: (path: string) => Promise<void>;
  loading: boolean;
  searching: boolean;
  filtered: FileEntry[];
  selected: FileEntry | null;
  onEntryClick: (entry: FileEntry) => void;
}) {
  return (
    <Card className="flex h-full min-h-0 flex-col">
      <CardHeader className="flex-col items-stretch gap-2 px-3.5 py-3">
        <div className="flex w-full flex-wrap items-center gap-1 text-[12px] text-zinc-500">
          {crumbs.map((crumb, index) => (
            <span key={crumb.path || "root"} className="flex items-center gap-1">
              {index > 0 ? <ChevronRight size={10} className="text-zinc-500" /> : null}
              <button
                type="button"
                onClick={() => void onLoadTree(crumb.path)}
                className={cn(
                  "flex items-center gap-1 rounded px-1 py-0.5 hover:bg-zinc-800 hover:text-zinc-200",
                  index === crumbs.length - 1 && "text-zinc-200",
                )}
              >
                {index === 0 ? <Home size={10} /> : null}
                {crumb.label}
              </button>
            </span>
          ))}
        </div>
        <FilterSearch
          value={filter}
          onChange={onFilterChange}
          placeholder={
            scope === "folder"
              ? "Filtrar nesta pasta"
              : "Procurar em todo o workspace"
          }
          ariaLabel={
            scope === "folder"
              ? "Filtrar arquivos nesta pasta"
              : "Procurar arquivos em todo o workspace"
          }
          className="min-w-0"
        />
        <div className="flex items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5 text-[12px]">
          <button
            type="button"
            onClick={() => onScopeChange("folder")}
            className={cn(
              "rounded-md px-2 py-1",
              scope === "folder"
                ? "workspace-tab-active"
                : "text-zinc-500 hover:text-zinc-200",
            )}
          >
            Nesta pasta
          </button>
          <button
            type="button"
            onClick={() => onScopeChange("workspace")}
            className={cn(
              "rounded-md px-2 py-1",
              scope === "workspace"
                ? "workspace-tab-active"
                : "text-zinc-500 hover:text-zinc-200",
            )}
          >
            Todo o workspace
          </button>
        </div>
      </CardHeader>
      <CardContent className="min-h-0 flex-1 overflow-y-auto p-1.5">
        {loading || searching ? (
          <LoadingState
            compact
            label={searching ? "Procurando no workspace…" : "Carregando pasta…"}
            rows={3}
          />
        ) : filtered.length === 0 ? (
          <EmptyState
            icon={FolderOpen}
            title={scope === "workspace" ? "Nada encontrado" : "Pasta vazia"}
            description={
              scope === "workspace"
                ? "Digite ao menos 2 caracteres do nome ou caminho."
                : "Nenhum arquivo visível nesta pasta."
            }
            className="border-0 bg-transparent py-8"
          />
        ) : (
          filtered.map((entry) => (
            <button
              key={entry.path}
              type="button"
              onClick={() => onEntryClick(entry)}
              className={cn(
                "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left transition-colors hover:bg-zinc-900",
                selected?.path === entry.path && "bg-zinc-800/80",
              )}
            >
              {entry.is_dir ? (
                <Folder size={14} className="shrink-0 text-amber-400/70" />
              ) : (
                <FileText size={14} className="shrink-0 text-zinc-500" />
              )}
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13px] text-zinc-300">
                  {entry.name}
                </span>
                <span className="block truncate font-mono text-[11.5px] text-zinc-500">
                  {entry.is_dir
                    ? "pasta"
                    : `${formatNumber(entry.size)} B · ${relativeTime(entry.modified)}`}
                </span>
              </span>
              {entry.is_dir ? (
                <ChevronRight size={12} className="text-zinc-500" />
              ) : null}
            </button>
          ))
        )}
      </CardContent>
    </Card>
  );
}
