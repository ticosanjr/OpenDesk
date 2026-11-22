import type { Dispatch, RefObject, SetStateAction } from "react";
import { Columns3, LayoutList, ListChecks, SlidersHorizontal, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { IconLabel } from "@/components/ui/icon-label";
import { FilterSearch } from "@/components/ui/filter-bar";
import { PRIORITY_LABEL } from "@/lib/task-ui";
import type { KanbanView } from "@/lib/kanban-views";
import {
  RETENTION_OPTIONS,
  type TaskVisibility,
} from "@/lib/task-visibility";
import { cn } from "@/lib/utils";

export interface KanbanToolbarProps {
  view: "board" | "list" | "queue";
  setView: (view: "board" | "list" | "queue") => void;
  query: string;
  setQuery: (value: string) => void;
  filtersRef: RefObject<HTMLDivElement | null>;
  filtersOpen: boolean;
  setFiltersOpen: Dispatch<SetStateAction<boolean>>;
  advancedFilterCount: number;
  agentFilter: string;
  setAgentFilter: Dispatch<SetStateAction<string>>;
  priorityFilter: string;
  setPriorityFilter: Dispatch<SetStateAction<string>>;
  attentionOnly: boolean;
  setAttentionOnly: Dispatch<SetStateAction<boolean>>;
  boardAgents: string[];
  filteredCount: number;
  totalCount: number;
  visibilityRef: RefObject<HTMLDivElement | null>;
  visibilityOpen: boolean;
  setVisibilityOpen: Dispatch<SetStateAction<boolean>>;
  visibility: TaskVisibility;
  onVisibilityChange: (next: TaskVisibility) => void;
  savedViews: KanbanView[];
  saveView: (view: Omit<KanbanView, "id">) => void;
  removeView: (id: string) => void;
  viewName: string;
  setViewName: Dispatch<SetStateAction<string>>;
  queueLength: number;
}

export function KanbanToolbar({
  view,
  setView,
  query,
  setQuery,
  filtersRef,
  filtersOpen,
  setFiltersOpen,
  advancedFilterCount,
  agentFilter,
  setAgentFilter,
  priorityFilter,
  setPriorityFilter,
  attentionOnly,
  setAttentionOnly,
  boardAgents,
  filteredCount,
  totalCount,
  visibilityRef,
  visibilityOpen,
  setVisibilityOpen,
  visibility,
  onVisibilityChange,
  savedViews,
  saveView,
  removeView,
  viewName,
  setViewName,
  queueLength,
}: KanbanToolbarProps) {
  function applySavedView(view: KanbanView) {
    setQuery(view.query);
    setAgentFilter(view.agent);
    setPriorityFilter(view.priority);
    setAttentionOnly(view.attentionOnly);
    setFiltersOpen(false);
  }

  function saveCurrentView() {
    const name = viewName.trim();
    if (!name) return;
    saveView({
      name,
      query,
      agent: agentFilter,
      priority: priorityFilter,
      attentionOnly,
    });
    setViewName("");
  }

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-3 py-2">
      <div className="flex items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
        {([
          ["board", Columns3, "Board"],
          ["list", LayoutList, "Lista"],
          ["queue", ListChecks, "Fila"],
        ] as const).map(([value, Icon, label]) => (
          <button
            key={value}
            type="button"
            onClick={() => setView(value)}
            className={cn(
              "flex items-center gap-1 rounded-md px-2 py-1 text-[12px] transition-colors",
              view === value
                ? "workspace-tab-active"
                : "text-zinc-500 hover:text-zinc-200",
            )}
            title={label}
            aria-pressed={view === value}
          >
            <Icon size={12} />
            {label}
          </button>
        ))}
      </div>
      {view !== "queue" ? (
        <>
          <FilterSearch
            value={query}
            onChange={setQuery}
            placeholder="Buscar por título, descrição, tag ou agente…"
            ariaLabel="Buscar tarefas"
          />
          <div ref={filtersRef} className="relative">
            <Button
              variant="outline"
              size="sm"
              className="h-8"
              onClick={() => setFiltersOpen((current) => !current)}
              aria-expanded={filtersOpen}
              title="Filtrar por agente, prioridade ou atenção"
            >
              <IconLabel icon={<SlidersHorizontal size={13} />}>
                Filtros{advancedFilterCount > 0 ? ` · ${advancedFilterCount}` : ""}
              </IconLabel>
            </Button>
            {filtersOpen ? (
              <div className="absolute right-0 top-9 z-40 max-h-[min(70dvh,520px)] w-[320px] space-y-3 overflow-y-auto rounded-xl border border-zinc-700 bg-zinc-950 p-3 shadow-2xl">
                <div className="flex items-center justify-between gap-2">
                  <p className="text-[13.5px] font-medium text-zinc-200">Filtros</p>
                  <button
                    type="button"
                    className="text-[13px] text-zinc-500 hover:text-zinc-200 disabled:opacity-40"
                    disabled={advancedFilterCount === 0}
                    onClick={() => {
                      setAgentFilter("");
                      setPriorityFilter("");
                      setAttentionOnly(false);
                    }}
                  >
                    limpar
                  </button>
                </div>
                <label className="block space-y-1">
                  <span className="text-[13px] text-zinc-400">Agente</span>
                  <select
                    value={agentFilter}
                    onChange={(event) => setAgentFilter(event.target.value)}
                    className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[13.5px] text-zinc-200"
                    aria-label="Filtrar por agente"
                  >
                    <option value="">todos os agentes</option>
                    {boardAgents.map((agent) => (
                      <option key={agent} value={agent}>
                        @{agent}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="block space-y-1">
                  <span className="text-[13px] text-zinc-400">Prioridade</span>
                  <select
                    value={priorityFilter}
                    onChange={(event) => setPriorityFilter(event.target.value)}
                    className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[13.5px] text-zinc-200"
                    aria-label="Filtrar por prioridade"
                  >
                    <option value="">todas as prioridades</option>
                    {[4, 3, 2, 1].map((priority) => (
                      <option key={priority} value={priority}>
                        {priority} — {PRIORITY_LABEL[priority]}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex items-center justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-2.5 py-2">
                  <span className="text-[13.5px] text-zinc-300">
                    Somente atenção
                  </span>
                  <input
                    type="checkbox"
                    checked={attentionOnly}
                    onChange={(event) => setAttentionOnly(event.target.checked)}
                    className="h-4 w-4 accent-blue-500"
                  />
                </label>
                <p className="text-[13px] leading-snug text-zinc-500">
                  Aprovação, pergunta, permissão ou falha. Bom para revisar o
                  que está parado.
                </p>
                <div className="space-y-2 border-t border-zinc-800/70 pt-3">
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-[13px] font-medium uppercase tracking-[0.08em] text-zinc-400">
                      Visões salvas
                    </p>
                    <span className="font-mono text-[13px] text-zinc-500">
                      {savedViews.length}
                    </span>
                  </div>
                  {savedViews.length === 0 ? (
                    <p className="text-[13px] leading-snug text-zinc-500">
                      Salve a combinação atual para reaplicar depois.
                    </p>
                  ) : (
                    <div className="space-y-1">
                      {savedViews.map((view) => (
                        <div
                          key={view.id}
                          className="flex items-center gap-1 rounded-md border border-zinc-800/70 bg-zinc-900/30"
                        >
                          <button
                            type="button"
                            onClick={() => applySavedView(view)}
                            className="min-w-0 flex-1 truncate px-2 py-1.5 text-left text-[13.5px] text-zinc-300 hover:text-zinc-100"
                          >
                            {view.name}
                          </button>
                          <button
                            type="button"
                            aria-label={`Remover visão ${view.name}`}
                            title="Remover visão"
                            onClick={() => removeView(view.id)}
                            className="rounded p-1.5 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
                          >
                            <X size={12} />
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                  <div className="flex gap-1.5">
                    <input
                      value={viewName}
                      onChange={(event) => setViewName(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") {
                          event.preventDefault();
                          saveCurrentView();
                        }
                      }}
                      placeholder="Nome da visão"
                      aria-label="Nome da visão"
                      className="h-8 min-w-0 flex-1 rounded-md border border-zinc-800 bg-zinc-900/60 px-2 text-[13.5px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
                    />
                    <Button
                      size="xs"
                      onClick={saveCurrentView}
                      disabled={!viewName.trim()}
                    >
                      Salvar
                    </Button>
                  </div>
                </div>
              </div>
            ) : null}
          </div>
          <Badge variant="default">
            {filteredCount} de {totalCount}
          </Badge>
          <div ref={visibilityRef} className="relative">
            <Button
              variant="outline"
              size="sm"
              className="h-8"
              onClick={() => setVisibilityOpen((current) => !current)}
              title="Escolher quanto tempo tarefas concluídas permanecem visíveis"
              aria-expanded={visibilityOpen}
            >
              <IconLabel icon={<SlidersHorizontal size={12} />}>
                Visibilidade
              </IconLabel>
            </Button>
            {visibilityOpen ? (
              <div className="absolute right-0 top-9 z-40 w-72 space-y-3 rounded-xl border border-zinc-700 bg-zinc-950 p-3 shadow-2xl">
                <div>
                  <p className="text-[13px] font-semibold text-zinc-200">
                    Concluídas e canceladas
                  </p>
                  <p className="text-[12px] leading-snug text-zinc-500">
                    Apenas visual. Nada é apagado do workspace.
                  </p>
                </div>
                <label className="block space-y-1">
                  <span className="text-[12px] text-zinc-400">Período</span>
                  <select
                    aria-label="Período de retenção"
                    value={visibility.retention}
                    onChange={(event) =>
                      onVisibilityChange({
                        ...visibility,
                        retention: event.target.value as TaskVisibility["retention"],
                      })
                    }
                    className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[13px] text-zinc-200"
                  >
                    {RETENTION_OPTIONS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label} — {option.hint}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="block space-y-1">
                  <span className="text-[12px] text-zinc-400">
                    Máximo de cards (0 = sem limite)
                  </span>
                  <input
                    aria-label="Máximo de cards concluídos"
                    type="number"
                    min={0}
                    max={200}
                    value={visibility.maxItems}
                    onChange={(event) =>
                      onVisibilityChange({
                        ...visibility,
                        maxItems: Math.max(
                          0,
                          Math.min(200, Number(event.target.value) || 0),
                        ),
                      })
                    }
                    className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[13px] text-zinc-200"
                  />
                </label>
                <div className="flex items-center justify-between gap-2 border-t border-zinc-800/70 pt-2">
                  <button
                    type="button"
                    className="text-[12px] text-zinc-500 hover:text-zinc-200"
                    onClick={() =>
                      onVisibilityChange({ retention: "all", maxItems: 0 })
                    }
                  >
                    mostrar tudo
                  </button>
                  <button
                    type="button"
                    className="text-[12px] text-zinc-500 hover:text-zinc-200"
                    onClick={() => setVisibilityOpen(false)}
                  >
                    fechar
                  </button>
                </div>
              </div>
            ) : null}
          </div>
        </>
      ) : (
        <span className="ml-auto text-[12px] text-zinc-500">
          {queueLength > 0
            ? `${queueLength} job(s) na fila`
            : "fila vazia"}
        </span>
      )}
    </div>
  );
}
