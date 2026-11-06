import { useEffect, useMemo, useState } from "react";
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
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { apiGet } from "@/lib/api";
import type { SearchHit } from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";

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
  const [tab, setTab] = useState<(typeof TABS)[number]["id"]>("all");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      const value = query.trim();
      setParams(value ? { q: value } : {}, { replace: true });
      if (!activeWorkspaceId || value.length < 2) {
        setHits([]);
        return;
      }
      setLoading(true);
      void apiGet<SearchHit[]>(
        `/api/search?workspace_id=${encodeURIComponent(activeWorkspaceId)}&q=${encodeURIComponent(value)}`,
      )
        .then((results) => {
          setHits(results);
          setError(null);
        })
        .catch((cause) => setError(cause instanceof Error ? cause.message : String(cause)))
        .finally(() => setLoading(false));
    }, 250);
    return () => window.clearTimeout(handle);
  }, [activeWorkspaceId, query, setParams]);

  const activeTab = TABS.find((item) => item.id === tab) ?? TABS[0];
  const filtered = useMemo(
    () =>
      activeTab.kinds
        ? hits.filter((hit) => (activeTab.kinds as readonly string[]).includes(hit.kind))
        : hits,
    [activeTab, hits],
  );

  function countFor(kinds: readonly string[] | null) {
    return kinds
      ? hits.filter((hit) => kinds.includes(hit.kind)).length
      : hits.length;
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="busca global"
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
            className="h-full flex-1 bg-transparent text-[14px] text-zinc-100 outline-none placeholder:text-zinc-600"
          />
          {loading ? <span className="text-[11px] text-zinc-500">buscando…</span> : null}
          <Badge variant="default">{filtered.length}</Badge>
        </div>
        <div className="mt-2 flex flex-wrap gap-1">
          {TABS.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => setTab(item.id)}
              className={cn(
                "flex items-center gap-1 rounded-lg px-2.5 py-1 text-[11px] font-medium transition-colors",
                tab === item.id
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200",
              )}
            >
              {item.label}
              <span className="font-mono text-[10px] text-zinc-600">
                {countFor(item.kinds)}
              </span>
            </button>
          ))}
        </div>
      </div>

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
          {error}
        </p>
      ) : null}

      {query.trim().length < 2 ? (
        <EmptyState
          icon={Search}
          title="Busque em todo o workspace"
          description="A busca cobre títulos, descrições, comentários, conteúdo textual de arquivos e metadados de execuções."
        />
      ) : filtered.length === 0 && !loading ? (
        <EmptyState
          icon={Search}
          title="Nada encontrado"
          description="Tente outro termo ou outra aba."
        />
      ) : (
        <div className="space-y-1.5">
          {filtered.map((hit) => {
            const meta = KIND_META[hit.kind] ?? {
              label: hit.kind,
              icon: FileText,
            };
            const Icon = meta.icon;
            return (
              <Link
                key={`${hit.kind}:${hit.id}`}
                to={hit.link}
                className="flex items-start gap-3 rounded-xl border border-zinc-800/70 bg-zinc-900/25 px-3.5 py-3 transition-colors hover:border-blue-500/30 hover:bg-zinc-900/50"
              >
                <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-950/50 text-zinc-500">
                  <Icon size={14} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <Badge variant="default">{meta.label}</Badge>
                    <p className="truncate text-[13px] font-semibold text-zinc-200">
                      {hit.title}
                    </p>
                  </div>
                  <p className="mt-1 line-clamp-2 text-[12px] leading-relaxed text-zinc-500">
                    {hit.snippet}
                  </p>
                </div>
                <span className="shrink-0 font-mono text-[10px] text-zinc-600">
                  {relativeTime(hit.updated_at)}
                </span>
              </Link>
            );
          })}
        </div>
      )}
    </PageBody>
  );
}
