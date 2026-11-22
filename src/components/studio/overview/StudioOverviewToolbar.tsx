import { Plus, RefreshCw, Search } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { EffectiveStudioItem } from "@/lib/types";
import { selectClass } from "./studio-overview-classes";

export function StudioOverviewToolbar({
  search,
  onSearchChange,
  kindFilter,
  onKindFilterChange,
  statusFilter,
  onStatusFilterChange,
  runtimeLocation,
  onRuntimeLocationChange,
  workspaceRoot,
  count,
  loading,
  onLoad,
  onInstall,
}: {
  search: string;
  onSearchChange: (value: string) => void;
  kindFilter: "all" | EffectiveStudioItem["kind"];
  onKindFilterChange: (value: "all" | EffectiveStudioItem["kind"]) => void;
  statusFilter: "all" | "enabled" | "disabled";
  onStatusFilterChange: (value: "all" | "enabled" | "disabled") => void;
  runtimeLocation: string;
  onRuntimeLocationChange: (value: string) => void;
  workspaceRoot: string;
  count: number;
  loading: boolean;
  onLoad: () => void;
  onInstall: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-zinc-800/70 px-4 py-3">
      <div className="flex h-9 min-w-[220px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5 focus-within:border-zinc-600">
        <Search size={13} className="text-zinc-500" />
        <input
          value={search}
          onChange={(event) => onSearchChange(event.target.value)}
          placeholder="Buscar por ID, nome ou descrição…"
          className="h-full flex-1 bg-transparent text-[14px] outline-none placeholder:text-zinc-500"
          aria-label="Buscar no Studio"
        />
      </div>
      <select
        aria-label="Filtrar por tipo"
        value={kindFilter}
        onChange={(event) =>
          onKindFilterChange(event.target.value as "all" | EffectiveStudioItem["kind"])
        }
        className={selectClass}
      >
        <option value="all">todos os tipos</option>
        <option value="agent">agentes</option>
        <option value="skill">skills</option>
        <option value="command">comandos</option>
        <option value="mcp">MCPs</option>
      </select>
      <select
        aria-label="Filtrar por estado"
        value={statusFilter}
        onChange={(event) =>
          onStatusFilterChange(event.target.value as "all" | "enabled" | "disabled")
        }
        className={selectClass}
      >
        <option value="all">todos os estados</option>
        <option value="enabled">ativos</option>
        <option value="disabled">desativados</option>
      </select>
      <div className="flex min-w-[260px] flex-1 items-center gap-1">
        <span className="text-[13px] uppercase tracking-wide text-zinc-500">
          runtime
        </span>
        <input
          value={runtimeLocation}
          onChange={(event) => onRuntimeLocationChange(event.target.value)}
          placeholder={workspaceRoot || "diretório do OpenCode TUI"}
          className="h-8 min-w-0 flex-1 rounded-md border border-zinc-800 bg-zinc-950/40 px-2.5 font-mono text-[11.5px] text-zinc-300 outline-none focus:border-blue-500/60"
          aria-label="Diretório de runtime do OpenCode"
        />
        <Button
          variant="ghost"
          size="xs"
          onClick={onLoad}
          disabled={loading}
        >
          carregar
        </Button>
      </div>
      <Badge variant="default">{count}</Badge>
      <Button variant="outline" size="sm" onClick={onLoad} disabled={loading}>
        <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
        Atualizar
      </Button>
      <Button size="sm" onClick={onInstall}>
        <Plus size={13} />
        Instalar
      </Button>
    </div>
  );
}
