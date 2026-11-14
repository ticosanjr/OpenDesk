import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  Activity,
  Archive,
  Bot,
  CalendarClock,
  FileText,
  FolderTree,
  KanbanSquare,
  MessageSquare,
  Search,
  X,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { apiGet } from "@/lib/api";
import type { SearchHit } from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import {
  loadRecentSearches,
  pushRecentSearch,
  saveRecentSearches,
} from "@/lib/recent-searches";

const TABS = [
  { id: "all", label: "Todos", kinds: null },
  { id: "tasks", label: "Tarefas", kinds: ["task"] },
  { id: "comments", label: "Comentários", kinds: ["comment"] },
  { id: "files", label: "Arquivos", kinds: ["file"] },
  { id: "artifacts", label: "Artefatos", kinds: ["artifact"] },
  { id: "agents", label: "Agentes", kinds: ["agent"] },
  { id: "projects", label: "Projetos", kinds: ["project"] },
  { id: "routines", label: "Rotinas", kinds: ["schedule"] },
  { id: "runs", label: "Execuções", kinds: ["run"] },
] as const;

const KIND_META: Record<string, { label: string; icon: typeof FileText }> = {
  task: { label: "Tarefa", icon: KanbanSquare },
  comment: { label: "Comentário", icon: MessageSquare },
  file: { label: "Arquivo", icon: FileText },
  artifact: { label: "Artefato", icon: Archive },
  agent: { label: "Agente", icon: Bot },
  project: { label: "Projeto", icon: FolderTree },
  schedule: { label: "Rotina", icon: CalendarClock },
  run: { label: "Execução", icon: Activity },
};

export function SearchPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const [params, setParams] = useSearchParams();
  const [query, setQuery] = useState(params.get("q") ?? "");
  // Track the last value this component wrote to the URL. Without it, the
  // debounced URL update echoes back an older query and swallows keystrokes.
  const lastWrittenQuery = useRef(params.get("q") ?? "");
  const [tab, setTab] = useState<(typeof TABS)[number]["id"]>(() => {
    const requested = params.get("tab");
    return TABS.some((item) => item.id === requested)
      ? (requested as (typeof TABS)[number]["id"])
      : "all";
  });
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [recentHits, setRecentHits] = useState<SearchHit[]>([]);
  const [recentLoading, setRecentLoading] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recent, setRecent] = useState<string[]>([]);
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  useEffect(() => {
    setRecent(loadRecentSearches(window.localStorage));
  }, []);

  useEffect(() => {
    if (!activeWorkspaceId || query.trim().length >= 2) return;
    let cancelled = false;
    const handle = window.setTimeout(() => {
      setRecentLoading(true);
      void apiGet<SearchHit[]>(
        `/api/search/recent?workspace_id=${encodeURIComponent(activeWorkspaceId)}&limit=24`,
      )
        .then((results) => {
          if (!cancelled) setRecentHits(Array.isArray(results) ? results : []);
        })
        .catch(() => {
          if (!cancelled) setRecentHits([]);
        })
        .finally(() => {
          if (!cancelled) setRecentLoading(false);
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    let cancelled = false;
    const handle = window.setTimeout(() => {
      // A pending debounce must not rewrite the hash after the user opened
      // another route from a search result.
      if (!window.location.hash.startsWith("#/search")) return;
      const value = query.trim();
      const next = new URLSearchParams();
      if (value) next.set("q", value);
      if (tab !== "all") next.set("tab", tab);
      lastWrittenQuery.current = value;
      setParams(next, { replace: true });
      if (!activeWorkspaceId || value.length < 2) {
        setHits([]);
        return;
      }
      setLoading(true);
      void apiGet<SearchHit[]>(
        `/api/search?workspace_id=${encodeURIComponent(activeWorkspaceId)}&q=${encodeURIComponent(value)}`,
      )
        .then((results) => {
          if (cancelled) return;
          setHits(results);
          setError(null);
        })
        .catch((cause) => {
          if (!cancelled) {
            setError(cause instanceof Error ? cause.message : String(cause));
          }
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [activeWorkspaceId, query, setParams, tab]);

  useEffect(() => {
    const nextQuery = params.get("q") ?? "";
    if (nextQuery === lastWrittenQuery.current) return;
    lastWrittenQuery.current = nextQuery;
    setQuery((current) => (current === nextQuery ? current : nextQuery));
  }, [params]);

  useEffect(() => {
    const requested = params.get("tab");
    if (requested && TABS.some((item) => item.id === requested) && requested !== tab) {
      setTab(requested as (typeof TABS)[number]["id"]);
    }
  }, [params, tab]);

  function remember(value: string) {
    setRecent((current) => {
      const next = pushRecentSearch(current, value);
      if (next === current) return current;
      saveRecentSearches(window.localStorage, next);
      return next;
    });
  }

  function selectTab(id: string) {
    const next = new URLSearchParams(params);
    if (id === "all") next.delete("tab");
    else next.set("tab", id);
    setTab(id as (typeof TABS)[number]["id"]);
    setParams(next, { replace: true });
  }

  const activeTab = TABS.find((item) => item.id === tab) ?? TABS[0];
  const filtered = useMemo(
    () =>
      activeTab.kinds
        ? hits.filter((hit) => (activeTab.kinds as readonly string[]).includes(hit.kind))
        : hits,
    [activeTab, hits],
  );

  const groups = useMemo(() => {
    if (tab !== "all") return [];
    return TABS.filter((item) => item.kinds)
      .map((item) => ({
        ...item,
        items: filtered.filter((hit) =>
          (item.kinds as readonly string[]).includes(hit.kind),
        ),
      }))
      .filter((group) => group.items.length > 0);
  }, [filtered, tab]);

  function countFor(kinds: readonly string[] | null) {
    return kinds
      ? hits.filter((hit) => kinds.includes(hit.kind)).length
      : hits.length;
  }

  return (
    <PageBody>
      <PageHeader
        title="Buscar"
        description={`${workspace?.name ?? "Workspace"} · tarefas, comentários, arquivos, artefatos, agentes, projetos, rotinas e execuções.`}
      />

      <div className="rounded-xl border border-zinc-800/70 bg-zinc-900/20 p-3">
        <div className="flex h-11 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/50 px-3">
          <Search size={16} className="text-zinc-500" />
          <input
            autoFocus
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Digite pelo menos 2 caracteres…"
            className="h-full flex-1 bg-transparent text-[14px] text-zinc-100 outline-none placeholder:text-zinc-500"
          />
          {loading ? <span className="text-[13px] text-zinc-500">buscando…</span> : null}
          {query ? (
            <button
              type="button"
              onClick={() => setQuery("")}
              aria-label="Limpar busca"
              title="Limpar busca"
              className="rounded p-0.5 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-200"
            >
              <X size={14} />
            </button>
          ) : null}
          <Badge variant="default">{filtered.length}</Badge>
        </div>
        {!query && recent.length > 0 ? (
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            <span className="font-mono text-[11.5px] uppercase tracking-wide text-zinc-500">
              recentes
            </span>
            {recent.map((item) => (
              <button
                key={item}
                type="button"
                onClick={() => {
                  setQuery(item);
                  selectTab("all");
                }}
                className="inline-flex h-7 items-center gap-1 rounded-md border border-zinc-800 bg-zinc-950/50 px-2 text-[12.5px] text-zinc-400 transition-colors hover:border-zinc-700 hover:text-zinc-100"
              >
                <Search size={10} />
                {item}
              </button>
            ))}
            <button
              type="button"
              className="text-[11.5px] text-zinc-500 hover:text-zinc-200"
              onClick={() => {
                setRecent([]);
                saveRecentSearches(window.localStorage, []);
              }}
            >
              limpar
            </button>
          </div>
        ) : null}
        {query.trim().length >= 2 ? (
        <div
          role="tablist"
          aria-label="Tipos de resultado"
          className="mt-2 flex flex-wrap gap-1"
        >
          {TABS.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={tab === item.id}
              onClick={() => selectTab(item.id)}
              className={cn(
                "flex items-center gap-1 rounded-lg px-2.5 py-1 text-[13px] font-medium transition-colors",
                tab === item.id
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200",
              )}
            >
              {item.label}
              <span className="font-mono text-[12px] text-zinc-500">
                {countFor(item.kinds)}
              </span>
            </button>
          ))}
        </div>
        ) : null}
      </div>

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13.5px] text-red-300">
          {error}
        </p>
      ) : null}

      {query.trim().length < 2 ? (
        recentHits.length > 0 ? (
          <section className="space-y-1.5">
            <header className="flex items-center gap-2 px-1">
              <Activity size={13} className="text-zinc-500" />
              <h2 className="text-[13px] font-medium uppercase tracking-[0.08em] text-zinc-400">
                Últimas atividades
              </h2>
              <span className="font-mono text-[13px] text-zinc-500">
                {recentHits.length}
              </span>
            </header>
            <div className="space-y-1.5">
              {recentHits.map((hit) => (
                <SearchHitRow
                  key={`recent:${hit.kind}:${hit.id}`}
                  hit={hit}
                  onOpen={() => remember(query)}
                />
              ))}
            </div>
          </section>
        ) : recentLoading ? (
          <p className="px-1 text-[13px] text-zinc-500">Carregando atividades recentes…</p>
        ) : (
          <EmptyState
            icon={Search}
            title="Busque em todo o workspace"
            description="A busca cobre títulos, descrições, comentários, conteúdo textual de arquivos e metadados de execuções."
          />
        )
      ) : filtered.length === 0 && !loading ? (
        <EmptyState
          icon={Search}
          title="Nada encontrado"
          description="Tente outro termo ou outra aba."
        />
      ) : tab === "all" && groups.length > 0 ? (
        <div className="space-y-4">
          {groups.map((group) => {
            const meta = KIND_META[group.kinds?.[0] ?? ""] ?? {
              label: group.label,
              icon: FileText,
            };
            const Icon = meta.icon;
            return (
              <section key={group.id} className="space-y-1.5">
                <header className="flex items-center gap-2 px-1">
                  <Icon size={13} className="text-zinc-500" />
                  <h2 className="text-[13px] font-medium uppercase tracking-[0.08em] text-zinc-400">
                    {group.label}
                  </h2>
                  <span className="font-mono text-[13px] text-zinc-500">
                    {group.items.length}
                  </span>
                </header>
                <div className="space-y-1.5">
                  {group.items.map((hit) => (
                    <SearchHitRow
                      key={`${hit.kind}:${hit.id}`}
                      hit={hit}
                      onOpen={() => remember(query)}
                    />
                  ))}
                </div>
              </section>
            );
          })}
        </div>
      ) : (
        <div className="space-y-1.5">
          {filtered.map((hit) => (
            <SearchHitRow
              key={`${hit.kind}:${hit.id}`}
              hit={hit}
              onOpen={() => remember(query)}
            />
          ))}
        </div>
      )}
    </PageBody>
  );
}


function SearchHitRow({
  hit,
  onOpen,
}: {
  hit: SearchHit;
  onOpen?: () => void;
}) {
  const meta = KIND_META[hit.kind] ?? { label: hit.kind, icon: FileText };
  const Icon = meta.icon;
  return (
    <Link
      to={hit.link}
      onClick={onOpen}
      className="flex items-start gap-3 rounded-xl border border-zinc-800/70 bg-zinc-900/25 px-3.5 py-3 transition-colors hover:border-blue-500/30 hover:bg-zinc-900/50"
    >
      <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-950/50 text-zinc-500">
        <Icon size={14} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="default">{meta.label}</Badge>
          <p className="truncate text-[13.5px] font-semibold text-zinc-200">
            {hit.title}
          </p>
        </div>
        <p className="mt-1 line-clamp-2 text-[13.5px] leading-relaxed text-zinc-400">
          {hit.snippet}
        </p>
      </div>
      <span className="shrink-0 font-mono text-[13px] text-zinc-500">
        {relativeTime(hit.updated_at)}
      </span>
    </Link>
  );
}
