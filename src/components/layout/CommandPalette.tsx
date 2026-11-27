import { useEffect, useMemo, useRef, useState } from "react";
import { Command } from "cmdk";
import { useNavigate } from "react-router-dom";
import {
  Activity,
  Archive,
  Brain,
  CalendarClock,
  CalendarDays,
  ChartNoAxesCombined,
  Clock3,
  FolderTree,
  Gauge,
  KanbanSquare,
  MessageSquare,
  Network,
  Plug,
  Plus,
  RefreshCw,
  Route,
  RotateCw,
  Search,
  Settings,
  Wand2,
  Workflow,
  X,
  type LucideIcon,
} from "lucide-react";
import { useUiStore } from "@/store/ui-store";
import { useAppStore } from "@/store/app-store";
import { apiGet } from "@/lib/api";
import type { SearchHit } from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { NAV_ITEMS } from "@/components/layout/Sidebar";
import {
  loadRecentSearches,
  pushRecentSearch,
  saveRecentSearches,
} from "@/lib/recent-searches";
import { cn } from "@/lib/utils";

const icons = {
  "/": Gauge,
  "/search": Search,
  "/kanban": KanbanSquare,
  "/calendar": CalendarDays,
  "/runs": Activity,
  "/chat": MessageSquare,
  "/files": FolderTree,
  "/routines": CalendarClock,
  "/recipes": Workflow,
  "/studio": Wand2,
  "/governance": Network,
  "/observability": ChartNoAxesCombined,
  "/learning": Brain,
  "/router": Route,
  "/integrations": Plug,
  "/settings": Settings,
} as const;

const GROUP_CLASS =
  "[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[12px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-500";

const ITEM_CLASS =
  "flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13.5px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100";

const HIT_META: Array<{
  kind: string;
  label: string;
  icon: LucideIcon;
}> = [
  { kind: "task", label: "Tarefas", icon: KanbanSquare },
  { kind: "run", label: "Execuções", icon: Activity },
  { kind: "artifact", label: "Artefatos", icon: Archive },
  { kind: "file", label: "Arquivos", icon: FolderTree },
  { kind: "agent", label: "Agentes", icon: Wand2 },
  { kind: "schedule", label: "Rotinas", icon: CalendarClock },
  { kind: "project", label: "Projetos", icon: FolderTree },
  { kind: "comment", label: "Comentários", icon: MessageSquare },
];

export function CommandPalette() {
  const open = useUiStore((state) => state.commandOpen);
  const setOpen = useUiStore((state) => state.setCommandOpen);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const refresh = useAppStore((state) => state.refresh);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recent, setRecent] = useState<string[]>([]);
  const searchSeq = useRef(0);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, setOpen]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setHits([]);
    setError(null);
    setRecent(loadRecentSearches(window.localStorage));
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const value = query.trim();
    const seq = searchSeq.current + 1;
    searchSeq.current = seq;
    if (!activeWorkspaceId || value.length < 2) {
      setHits([]);
      setSearching(false);
      setError(null);
      return;
    }
    setSearching(true);
    const handle = window.setTimeout(() => {
      void apiGet<SearchHit[]>(
        `/api/search?workspace_id=${encodeURIComponent(activeWorkspaceId)}&q=${encodeURIComponent(value)}`,
      )
        .then((results) => {
          if (searchSeq.current !== seq) return;
          setHits(Array.isArray(results) ? results : []);
          setError(null);
        })
        .catch((cause) => {
          if (searchSeq.current !== seq) return;
          setHits([]);
          setError(cause instanceof Error ? cause.message : String(cause));
        })
        .finally(() => {
          if (searchSeq.current === seq) setSearching(false);
        });
    }, 180);
    return () => window.clearTimeout(handle);
  }, [activeWorkspaceId, open, query]);

  const remember = (value: string) => {
    const next = pushRecentSearch(recent, value);
    setRecent(next);
    saveRecentSearches(window.localStorage, next);
  };

  const groups = useMemo(() => {
    if (query.trim().length < 2) return [];
    return HIT_META.map((meta) => ({
      ...meta,
      hits: hits.filter((hit) => hit.kind === meta.kind).slice(0, 6),
    })).filter((group) => group.hits.length > 0);
  }, [hits, query]);

  if (!open) return null;

  function run(action: () => void) {
    setOpen(false);
    action();
  }

  function openFullSearch(value: string) {
    const text = value.trim();
    if (text.length >= 2) remember(text);
    run(() => navigate(`/search?q=${encodeURIComponent(text)}`));
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <button
        type="button"
        aria-label="Fechar command bar"
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={() => setOpen(false)}
      />
      <Command
        label="Busca global e comandos"
        role="dialog"
        aria-modal="true"
        shouldFilter={false}
        className="relative w-full max-w-4xl overflow-hidden rounded-2xl border border-zinc-800 bg-zinc-950/95 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] backdrop-blur-xl"
      >
        <div className="flex items-center gap-2 border-b border-zinc-800/70 px-3">
          <Search size={15} className="text-zinc-500" />
          <Command.Input
            autoFocus
            value={query}
            onValueChange={setQuery}
            placeholder="Buscar em todo o workspace ou executar uma ação…"
            className="h-11 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
          />
          {searching ? (
            <span className="font-mono text-[11.5px] text-zinc-500">buscando…</span>
          ) : null}
          <span className="rounded border border-zinc-800 bg-zinc-900 px-1.5 py-0.5 font-mono text-[12px] text-zinc-500">
            ESC
          </span>
        </div>

        <Command.List className="max-h-[min(60dvh,28rem)] overflow-y-auto p-2">
          <Command.Empty className="px-3 py-8 text-center text-xs text-zinc-500">
            {query.trim().length >= 2
              ? "Nenhum resultado. Use a busca completa para filtrar por tipo."
              : "Nenhum resultado."}
          </Command.Empty>

          {query.trim().length >= 2 ? (
            <Command.Group heading="Busca completa" className={GROUP_CLASS}>
              <Command.Item
                value={`buscar ${query.trim()} em todo o workspace`}
                onSelect={() => openFullSearch(query)}
                className={ITEM_CLASS}
              >
                <Search size={15} className="shrink-0 text-zinc-500" />
                <span className="min-w-0 flex-1 truncate">
                  Ver todos os resultados para “{query.trim()}”
                </span>
                <span className="shrink-0 font-mono text-[12px] text-zinc-500">
                  Enter
                </span>
              </Command.Item>
            </Command.Group>
          ) : recent.length > 0 ? (
            <Command.Group heading="Buscas recentes" className={GROUP_CLASS}>
              {recent.map((item) => (
                <Command.Item
                  key={item}
                  value={`recente ${item}`}
                  onSelect={() => setQuery(item)}
                  className={ITEM_CLASS}
                >
                  <Clock3 size={14} className="shrink-0 text-zinc-500" />
                  <span className="min-w-0 flex-1 truncate">{item}</span>
                  <button
                    type="button"
                    aria-label={`Remover ${item} das buscas recentes`}
                    className="rounded p-0.5 text-zinc-600 hover:text-zinc-300"
                    onClick={(event) => {
                      event.stopPropagation();
                      const next = recent.filter((value) => value !== item);
                      setRecent(next);
                      saveRecentSearches(window.localStorage, next);
                    }}
                  >
                    <X size={12} />
                  </button>
                </Command.Item>
              ))}
            </Command.Group>
          ) : null}

          {groups.map((group) => {
            const Icon = group.icon;
            return (
              <Command.Group
                key={group.kind}
                heading={group.label}
                className={cn("mt-2", GROUP_CLASS)}
              >
                {group.hits.map((hit) => (
                  <Command.Item
                    key={`${hit.kind}-${hit.id}`}
                    value={`${group.label} ${hit.title} ${hit.snippet}`}
                    onSelect={() =>
                      run(() => {
                        remember(query);
                        navigate(hit.link);
                      })
                    }
                    className={ITEM_CLASS}
                  >
                    <Icon size={14} className="shrink-0 text-zinc-500" />
                    <span className="min-w-0 flex-1 truncate">{hit.title}</span>
                    {hit.updated_at ? (
                      <span className="shrink-0 font-mono text-[11.5px] text-zinc-500">
                        {relativeTime(hit.updated_at)}
                      </span>
                    ) : null}
                  </Command.Item>
                ))}
              </Command.Group>
            );
          })}

          {error ? (
            <p className="mx-2 mt-2 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-200">
              {error}
            </p>
          ) : null}

          {query.trim().length < 2 ? (
            <>
              <Command.Group heading="Navegação" className={cn("mt-2", GROUP_CLASS)}>
                {NAV_ITEMS.map((item) => {
                  const Icon = icons[item.to as keyof typeof icons] ?? Gauge;
                  return (
                    <Command.Item
                      key={item.to}
                      value={item.label}
                      onSelect={() => run(() => navigate(item.to))}
                      className={cn(ITEM_CLASS, "text-[14px]")}
                    >
                      <Icon size={15} className="text-zinc-500" />
                      {item.label}
                    </Command.Item>
                  );
                })}
              </Command.Group>

              <Command.Group heading="Ações" className={cn("mt-2", GROUP_CLASS)}>
                <Command.Item
                  value="Nova tarefa"
                  onSelect={() =>
                    run(() => {
                      navigate("/kanban");
                      setTaskDialogOpen(true);
                    })
                  }
                  className={cn(ITEM_CLASS, "text-[14px]")}
                >
                  <KanbanSquare size={15} className="text-zinc-500" />
                  Nova tarefa
                </Command.Item>
                <Command.Item
                  value="Abrir execuções"
                  onSelect={() => run(() => navigate("/runs"))}
                  className={cn(ITEM_CLASS, "text-[14px]")}
                >
                  <Activity size={15} className="text-zinc-500" />
                  Abrir execuções
                </Command.Item>
                <Command.Item
                  value="Novo workspace"
                  onSelect={() => run(() => setWorkspaceDialogOpen(true))}
                  className={cn(ITEM_CLASS, "text-[14px]")}
                >
                  <Plus size={15} className="text-zinc-500" />
                  Novo workspace
                </Command.Item>
                <Command.Item
                  value="Reiniciar OpenCode Daemon"
                  onSelect={() => run(() => void restartDaemon())}
                  className={cn(ITEM_CLASS, "text-[14px]")}
                >
                  <RotateCw size={15} className="text-zinc-500" />
                  Reiniciar OpenCode Daemon
                </Command.Item>
                <Command.Item
                  value="Atualizar dados"
                  onSelect={() => run(() => void refresh())}
                  className={cn(ITEM_CLASS, "text-[14px]")}
                >
                  <RefreshCw size={15} className="text-zinc-500" />
                  Atualizar dados
                </Command.Item>
              </Command.Group>
            </>
          ) : null}
        </Command.List>

        <div className="flex items-center justify-between border-t border-zinc-800/70 px-3 py-2 font-mono text-[12px] text-zinc-500">
          <span>↑↓ navegar · ↵ selecionar</span>
          <span>OpenDesk</span>
        </div>
      </Command>
    </div>
  );
}
