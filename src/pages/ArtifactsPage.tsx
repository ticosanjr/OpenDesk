import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  Archive,
  ExternalLink,
  FileText,
  Filter,
  GitCompare,
  Mail,
  RefreshCw,
  RotateCcw,
  Search,
  Star,
  X,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet, apiPost } from "@/lib/api";
import type { Artifact } from "@/lib/types";
import { formatNumber, relativeTime, shortPath } from "@/lib/format";
import { cn } from "@/lib/utils";
import { links } from "@/lib/links";
import { EntityLink } from "@/components/ui/entity-link";
import { LazyFileViewer } from "@/components/viewer/LazyFileViewer";
import { toast } from "@/store/toast-store";
import { confirmAction } from "@/store/confirm-store";

interface ArtifactContent {
  path: string;
  content: string;
  truncated: boolean;
  binary: boolean;
  mime: string;
  size: number;
}

export function ArtifactsPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const lastArtifactEvent = useAppStore((state) => state.lastArtifactEvent);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );

  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [agentFilter, setAgentFilter] = useState("");
  const [debouncedAgent, setDebouncedAgent] = useState("");
  const [limit, setLimit] = useState(200);
  const [actionFilter, setActionFilter] = useState("");
  const [selected, setSelected] = useState<Artifact | null>(null);
  const [content, setContent] = useState<ArtifactContent | null>(null);
  const [tab, setTab] = useState<"content" | "diff">("content");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();

  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  useEffect(() => {
    const handle = window.setTimeout(() => setDebouncedAgent(agentFilter), 250);
    return () => window.clearTimeout(handle);
  }, [agentFilter]);

  const runFilter = searchParams.get("run") ?? "";
  const query = useMemo(() => {
    if (!activeWorkspaceId) return "";
    const params = new URLSearchParams({ workspace_id: activeWorkspaceId, limit: String(limit) });
    if (debouncedAgent.trim()) params.set("agent", debouncedAgent.trim());
    if (actionFilter) params.set("action", actionFilter);
    if (runFilter) params.set("run_id", runFilter);
    return params.toString();
  }, [activeWorkspaceId, debouncedAgent, actionFilter, limit, runFilter]);

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setArtifacts([]);
      return;
    }
    setLoading(true);
    try {
      const items = await apiGet<Artifact[]>(`/api/artifacts?${query}`);
      setArtifacts(items);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    void load();
  }, [load, lastArtifactEvent]);

  // Deep link from the dashboard: /artifacts?id=<artifact-id>
  useEffect(() => {
    const id = searchParams.get("id");
    if (!id || artifacts.length === 0) return;
    const artifact = artifacts.find((item) => item.id === id);
    if (artifact) {
      setSelected(artifact);
      setSearchParams((current) => {
        const next = new URLSearchParams(current);
        next.delete("id");
        return next;
      }, { replace: true });
    }
  }, [artifacts, searchParams, setSearchParams]);

  useEffect(() => {
    if (!selected) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSelected(null);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [selected]);

  useEffect(() => {
    if (!selected) {
      setContent(null);
      return;
    }
    let cancelled = false;
    apiGet<ArtifactContent>(`/api/artifacts/${selected.id}/content`)
      .then((value) => {
        if (!cancelled) setContent(value);
      })
      .catch((cause) => {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
          setContent(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [selected]);

  async function togglePin(artifact: Artifact) {
    try {
      const updated = await apiPost<Artifact>(
        `/api/artifacts/${artifact.id}/pin`,
        { pinned: !artifact.pinned },
      );
      setArtifacts((current) =>
        current.map((item) => (item.id === updated.id ? updated : item)),
      );
      setSelected((current) => (current?.id === updated.id ? updated : current));
      toast({
        tone: "success",
        title: updated.pinned ? "Artefato fixado" : "Artefato desafixado",
        href: links.artifact(updated.id),
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível fixar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function rollback(artifact: Artifact) {
    const confirmed = await confirmAction({
      title: "Reverter esta alteração?",
      description:
        artifact.action_type === "CREATED"
          ? "O arquivo criado será removido do workspace."
          : "Os trechos alterados voltam ao conteúdo anterior. Se o arquivo mudou depois, o rollback é recusado.",
      confirmLabel: "Reverter",
      tone: "destructive",
    });
    if (!confirmed) return;
    try {
      await apiPost(`/api/artifacts/${artifact.id}/rollback`, {});
      toast({ tone: "success", title: "Alteração revertida", href: links.artifact(artifact.id) });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Rollback não aplicado",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function reveal(path: string) {
    try {
      await openPath(path);
    } catch (cause) {
      console.warn("OpenDesk: could not open path", cause);
    }
  }

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader eyebrow="proveniência" title="Artefatos" />
        <EmptyState
          icon={Archive}
          title="Nenhum workspace ativo"
          description="Selecione um workspace para ver os arquivos produzidos pelos agentes."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="proveniência"
        title="Artefatos"
        description={`${workspace?.name ?? "Workspace"} · quem criou, por quê, quando, diff e destinos.`}
        actions={
          <Button variant="outline" size="sm" onClick={() => void load()}>
            <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
            Atualizar
          </Button>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      <Card>
        <CardHeader className="gap-3">
          <CardTitle>Explorador de proveniência</CardTitle>
          <div className="flex flex-wrap items-center gap-2">
            <div className="flex h-8 min-w-[220px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5">
              <Search size={13} className="text-zinc-500" />
              <input
                value={agentFilter}
                onChange={(event) => setAgentFilter(event.target.value)}
                placeholder="Filtrar por agente (@build, @auditor…)"
                className="h-full flex-1 bg-transparent text-[11px] outline-none placeholder:text-zinc-500"
              />
            </div>
            <div className="flex h-8 items-center gap-1.5 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5">
              <Filter size={12} className="text-zinc-500" />
              <select
                value={actionFilter}
                onChange={(event) => setActionFilter(event.target.value)}
                className="bg-transparent text-[11px] text-zinc-300 outline-none"
              >
                <option value="">toda ação</option>
                <option value="CREATED">criado</option>
                <option value="MODIFIED">modificado</option>
                <option value="DELETED">deletado</option>
              </select>
            </div>
            <Badge variant="default">{artifacts.length} artefatos</Badge>
            {runFilter ? (
              <button
                type="button"
                onClick={() =>
                  setSearchParams((current) => {
                    const next = new URLSearchParams(current);
                    next.delete("run");
                    return next;
                  }, { replace: true })
                }
                className="inline-flex items-center gap-1 rounded-md border border-blue-500/25 bg-blue-500/10 px-1.5 py-0.5 font-mono text-[9px] text-blue-300 hover:text-blue-100"
                title="Remover filtro por execução"
              >
                run {runFilter.slice(0, 8)}
                <X size={9} />
              </button>
            ) : null}
          </div>
        </CardHeader>
        <CardContent className="p-0">
          {artifacts.length === 0 ? (
            <div className="p-4">
              <EmptyState
                icon={Archive}
                title="Nenhum artefato registrado"
                description="Quando um run criar ou modificar arquivos, o stream interceptor grava autor, motivo, timestamp e diff."
              />
            </div>
          ) : (
            <div className="scroll-area max-h-[calc(100dvh-18rem)] overflow-auto">
              <table className="w-full text-left">
                <thead className="border-b border-zinc-800/70 text-[9px] uppercase tracking-[0.12em] text-zinc-500">
                  <tr>
                    <th className="px-4 py-2.5 font-medium">Arquivo</th>
                    <th className="px-3 py-2.5 font-medium">Agente</th>
                    <th className="px-3 py-2.5 font-medium">Motivo</th>
                    <th className="px-3 py-2.5 font-medium">Quando</th>
                    <th className="px-3 py-2.5 font-medium">Ação</th>
                    <th className="px-3 py-2.5 font-medium">Destinos</th>
                  </tr>
                </thead>
                <tbody>
                  {artifacts.map((artifact) => (
                    <tr
                      key={artifact.id}
                      className="cursor-pointer border-b border-zinc-800/40 text-[11px] transition-colors hover:bg-zinc-900/40"
                      onClick={() => {
                        setSelected(artifact);
                        setTab("content");
                      }}
                    >
                      <td className="px-4 py-2.5">
                        <div className="flex items-center gap-2">
                          <button
                            type="button"
                            title={artifact.pinned ? "Desafixar entregável" : "Fixar como entregável"}
                            onClick={(event) => {
                              event.stopPropagation();
                              void togglePin(artifact);
                            }}
                            className={artifact.pinned ? "text-amber-300" : "text-zinc-500 hover:text-amber-300"}
                          >
                            <Star size={13} fill={artifact.pinned ? "currentColor" : "none"} />
                          </button>
                          <FileText size={13} className="shrink-0 text-zinc-500" />
                          <div className="min-w-0">
                            <p className="truncate font-mono text-[10px] text-zinc-200">
                              {artifact.file_path}
                            </p>
                            <p className="font-mono text-[8px] text-zinc-500">
                              {formatNumber(artifact.file_size_bytes)} bytes ·{" "}
                              {artifact.file_type || "file"}
                            </p>
                          </div>
                        </div>
                      </td>
                      <td className="px-3 py-2.5">
                        <Badge variant="purple">@{artifact.created_by_agent}</Badge>
                      </td>
                      <td className="max-w-[260px] px-3 py-2.5">
                        <p className="truncate text-[10px] text-zinc-500">
                          {artifact.creation_reason}
                        </p>
                      </td>
                      <td className="px-3 py-2.5">
                        <span className="font-mono text-[9px] text-zinc-500">
                          {relativeTime(artifact.created_at)}
                        </span>
                      </td>
                      <td className="px-3 py-2.5">
                        <Badge
                          variant={
                            artifact.action_type === "CREATED"
                              ? "green"
                              : artifact.action_type === "DELETED"
                                ? "red"
                                : "yellow"
                          }
                        >
                          {artifact.action_type}
                        </Badge>
                      </td>
                      <td className="px-3 py-2.5">
                        <div className="flex gap-1">
                          {artifact.google_doc_url ? <Badge variant="blue">docs</Badge> : null}
                          {artifact.google_drive_url ? <Badge variant="blue">drive</Badge> : null}
                          {artifact.emailed_to ? <Badge variant="blue">email</Badge> : null}
                          {!artifact.google_doc_url &&
                          !artifact.google_drive_url &&
                          !artifact.emailed_to ? (
                            <span className="text-[9px] text-zinc-500">—</span>
                          ) : null}
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {artifacts.length >= limit ? (
            <div className="border-t border-zinc-800/60 p-3 text-center">
              <Button variant="outline" size="sm" onClick={() => setLimit((value) => value + 200)}>
                Carregar mais artefatos
              </Button>
            </div>
          ) : null}
        </CardContent>
      </Card>

      {selected ? (
        <div className="fixed inset-0 z-40 flex items-center justify-center p-3 sm:p-5">
          <button
            type="button"
            aria-label="Fechar artefato"
            className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
            onClick={() => setSelected(null)}
          />
          <aside className="relative flex h-[min(88dvh,64rem)] w-[min(94vw,72rem)] flex-col overflow-hidden rounded-2xl border border-zinc-800 bg-zinc-950/95 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] backdrop-blur-xl">
            <div className="flex items-start justify-between gap-3 border-b border-zinc-800/70 px-4 py-3">
              <div className="min-w-0">
                <p className="truncate font-mono text-[11px] text-zinc-200">
                  {selected.file_path}
                </p>
                <p className="mt-0.5 truncate font-mono text-[9px] text-zinc-500">
                  {shortPath(selected.absolute_path, 90)}
                </p>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  <EntityLink
                    to={links.agent(
                      selected.created_by_agent,
                      "workspace",
                      selected.workspace_id,
                    )}
                    chip
                  >
                    @{selected.created_by_agent}
                  </EntityLink>
                  <Badge variant="default">{selected.action_type}</Badge>
                  <Badge variant="default">{relativeTime(selected.created_at)}</Badge>
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void togglePin(selected)}
                  title={selected.pinned ? "Desafixar entregável" : "Fixar como entregável"}
                >
                  <Star size={12} fill={selected.pinned ? "currentColor" : "none"} />
                  {selected.pinned ? "Fixado" : "Fixar"}
                </Button>
                {selected.diff_patch || selected.action_type === "CREATED" ? (
                  <Button
                    variant="outline"
                    size="sm"
                    className="border-amber-500/30 text-amber-200"
                    onClick={() => void rollback(selected)}
                    title="Reverter esta alteração no arquivo"
                  >
                    <RotateCcw size={12} />
                    Reverter
                  </Button>
                ) : null}
                {selected.run_id ? (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => navigate(links.run(selected.run_id ?? ""))}
                  >
                    Ver execução
                  </Button>
                ) : null}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void reveal(selected.absolute_path)}
                >
                  <ExternalLink size={12} />
                  Abrir pasta
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Fechar visualizador"
                  onClick={() => setSelected(null)}
                  className="h-8 w-8"
                >
                  <X size={15} />
                </Button>
              </div>
            </div>

            <div className="flex items-center gap-1 border-b border-zinc-800/70 px-3 py-1.5">
              <button
                type="button"
                onClick={() => setTab("content")}
                className={cn(
                  "flex items-center gap-1.5 rounded-md px-2 py-1 text-[10px]",
                  tab === "content"
                    ? "bg-zinc-800 text-zinc-100"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                <FileText size={12} />
                Conteúdo
              </button>
              <button
                type="button"
                onClick={() => setTab("diff")}
                className={cn(
                  "flex items-center gap-1.5 rounded-md px-2 py-1 text-[10px]",
                  tab === "diff"
                    ? "bg-zinc-800 text-zinc-100"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                <GitCompare size={12} />
                Diff
              </button>
              <div className="ml-auto flex gap-1">
                <Button variant="ghost" size="sm" className="h-6 px-1.5 text-[9px]" disabled>
                  <Mail size={10} />
                  Enviar
                </Button>
                <Button variant="ghost" size="sm" className="h-6 px-1.5 text-[9px]" disabled>
                  Google Docs
                </Button>
              </div>
            </div>

            <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
              {tab === "content" ? (
                content ? (
                  <LazyFileViewer
                    name={selected.file_path}
                    path={content.path}
                    content={content.content}
                    binary={content.binary}
                    mime={content.mime}
                    truncated={content.truncated}
                    size={content.size}
                    rawUrl={`/api/artifacts/${selected.id}/raw`}
                    onOpenExternal={() => void reveal(selected.absolute_path)}
                  />
                ) : (
                  <p className="p-4 text-[11px] text-zinc-500">
                    {loading ? "Carregando visualizador…" : "Sem prévia para este artefato."}
                  </p>
                )
              ) : (
                <div className="min-h-0 flex-1 overflow-auto p-4">
                  <p className="mb-2 text-[11px] leading-relaxed text-zinc-500">
                    {selected.creation_reason}
                  </p>
                  <pre className="whitespace-pre-wrap break-words rounded-lg border border-zinc-800/70 bg-zinc-900/30 p-3 font-mono text-[11px] leading-relaxed text-zinc-300">
                    {selected.diff_patch || "Sem diff registrado para este artefato."}
                  </pre>
                </div>
              )}
            </div>
          </aside>
        </div>
      ) : null}
    </PageBody>
  );
}
