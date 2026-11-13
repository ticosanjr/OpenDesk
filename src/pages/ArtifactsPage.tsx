import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  Archive,
  Download,
  ExternalLink,
  FileText,
  GitCompare,
  RefreshCw,
  RotateCcw,
  Star,
  X,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { FilterSearch, FilterSelect } from "@/components/ui/filter-bar";
import { Modal } from "@/components/ui/modal";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet, apiPost } from "@/lib/api";
import type { Artifact } from "@/lib/types";
import { formatNumber, relativeTime, shortPath } from "@/lib/format";
import { saveLocalFile } from "@/lib/save-file";
import { cn } from "@/lib/utils";
import { links } from "@/lib/links";
import { EntityLink } from "@/components/ui/entity-link";
import { LazyFileViewer } from "@/components/viewer/LazyFileViewer";
import type { ArchiveEntry } from "@/components/viewer/FileViewer";
import { DiffView } from "@/components/viewer/DiffView";
import { toast } from "@/store/toast-store";
import { confirmAction } from "@/store/confirm-store";

function artifactActionLabel(action: string): string {
  if (action === "CREATED") return "criado";
  if (action === "DELETED") return "excluído";
  if (action === "MODIFIED") return "alterado";
  return action;
}

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
  const [contentLoading, setContentLoading] = useState(false);
  const [contentError, setContentError] = useState<string | null>(null);
  const [archive, setArchive] = useState<ArchiveEntry[] | null>(null);
  const [archiveLoading, setArchiveLoading] = useState(false);
  const [archiveError, setArchiveError] = useState<string | null>(null);
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
  const taskFilter = searchParams.get("task") ?? "";
  const query = useMemo(() => {
    if (!activeWorkspaceId) return "";
    const params = new URLSearchParams({ workspace_id: activeWorkspaceId, limit: String(limit) });
    if (debouncedAgent.trim()) params.set("agent", debouncedAgent.trim());
    if (actionFilter) params.set("action", actionFilter);
    if (runFilter) params.set("run_id", runFilter);
    if (taskFilter) params.set("task_id", taskFilter);
    return params.toString();
  }, [activeWorkspaceId, debouncedAgent, actionFilter, limit, runFilter, taskFilter]);

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

  // Deep link from the dashboard/search: /artifacts?id=<artifact-id>.
  // Fetch by id instead of depending on the current list/filter window.
  useEffect(() => {
    const id = searchParams.get("id");
    if (!id) return;
    let cancelled = false;
    apiGet<Artifact>(`/api/artifacts/${id}`)
      .then((artifact) => {
        if (cancelled) return;
        setSelected(artifact);
        setArtifacts((current) =>
          current.some((item) => item.id === artifact.id)
            ? current
            : [artifact, ...current],
        );
        setSearchParams(
          (current) => {
            const next = new URLSearchParams(current);
            next.delete("id");
            return next;
          },
          { replace: true },
        );
      })
      .catch((cause) => {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [searchParams, setSearchParams]);

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
      setContentError(null);
      setContentLoading(false);
      return;
    }
    let cancelled = false;
    setContent(null);
    setContentError(null);
    setContentLoading(true);
    apiGet<ArtifactContent>(`/api/artifacts/${selected.id}/content`)
      .then((value) => {
        if (!cancelled) setContent(value);
      })
      .catch((cause) => {
        if (!cancelled) {
          setContentError(cause instanceof Error ? cause.message : String(cause));
          setContent(null);
        }
      })
      .finally(() => {
        if (!cancelled) setContentLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selected]);

  useEffect(() => {
    const path =
      content?.binary && /\.zip$/i.test(content.path) ? content.path : null;
    setArchive(null);
    setArchiveError(null);
    if (!path || !selected) {
      setArchiveLoading(false);
      return;
    }
    let cancelled = false;
    setArchiveLoading(true);
    void apiGet<ArchiveEntry[]>(`/api/artifacts/${selected.id}/archive`)
      .then((list) => {
        if (!cancelled) setArchive(Array.isArray(list) ? list : []);
      })
      .catch((cause) => {
        if (!cancelled) {
          setArchiveError(cause instanceof Error ? cause.message : String(cause));
        }
      })
      .finally(() => {
        if (!cancelled) setArchiveLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [content, selected]);

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

  function containingFolder(path: string): string {
    const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return index > 0 ? path.slice(0, index) : path;
  }

  async function saveArtifact(artifact: Artifact) {
    const filename =
      artifact.file_path.split("/").pop() ?? "artefato";
    try {
      const saved = await saveLocalFile({
        filename,
        artifactId: artifact.id,
        fallbackUrl: `/api/artifacts/${artifact.id}/raw`,
      });
      if (saved) {
        toast({
          tone: "success",
          title: "Download salvo",
          description: filename,
        });
      }
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível baixar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader title="Artefatos" />
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
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}

      <Card>
        <CardHeader className="gap-3">
          <CardTitle>Explorador de proveniência</CardTitle>
          <div className="flex flex-wrap items-center gap-2">
            <FilterSearch
              value={agentFilter}
              onChange={setAgentFilter}
              placeholder="Filtrar por agente…"
              ariaLabel="Filtrar artefatos por agente"
              className="min-w-[220px]"
            />
            <FilterSelect
              value={actionFilter}
              onChange={setActionFilter}
              ariaLabel="Filtrar artefatos por ação"
            >
              <option value="">toda ação</option>
              <option value="CREATED">criado</option>
              <option value="MODIFIED">modificado</option>
              <option value="DELETED">deletado</option>
            </FilterSelect>
            <Badge variant="default">
              {artifacts.length} {artifacts.length === 1 ? "artefato" : "artefatos"}
            </Badge>
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
                className="inline-flex items-center gap-1 rounded-md border border-blue-500/25 bg-blue-500/10 px-1.5 py-0.5 font-mono text-[11.5px] text-blue-300 hover:text-blue-100"
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
                compact
              />
            </div>
          ) : (
            <div className="scroll-area max-h-[calc(100dvh-18rem)] overflow-auto">
              <table className="w-full text-left">
                <thead className="border-b border-zinc-800/70 text-[11.5px] uppercase tracking-[0.12em] text-zinc-500">
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
                      className="cursor-pointer border-b border-zinc-800/40 text-[13px] transition-colors hover:bg-zinc-900/40"
                      onClick={() => {
                        setSelected(artifact);
                        setTab("content");
                      }}
                    >
                      <td className="px-4 py-2.5">
                        <div className="flex items-center gap-2">
                          <button
                            type="button"
                            title={`Baixar ${artifact.file_path}`}
                            aria-label={`Baixar ${artifact.file_path}`}
                            onClick={(event) => {
                              event.stopPropagation();
                              void saveArtifact(artifact);
                            }}
                            className="-m-1 rounded p-1 text-zinc-500 transition-colors hover:text-zinc-100"
                          >
                            <Download size={13} />
                          </button>
                          <button
                            type="button"
                            title={artifact.pinned ? "Desafixar entregável" : "Fixar como entregável"}
                            aria-label={artifact.pinned ? "Desafixar entregável" : "Fixar como entregável"}
                            onClick={(event) => {
                              event.stopPropagation();
                              void togglePin(artifact);
                            }}
                            className={cn(
                              "-m-1 rounded p-1 transition-colors",
                              artifact.pinned
                                ? "text-amber-300"
                                : "text-zinc-500 hover:text-amber-300",
                            )}
                          >
                            <Star size={13} fill={artifact.pinned ? "currentColor" : "none"} />
                          </button>
                          <FileText size={13} className="shrink-0 text-zinc-500" />
                          <div className="min-w-0">
                            <p className="truncate font-mono text-[12px] text-zinc-200">
                              {artifact.file_path}
                            </p>
                            <p className="font-mono text-[11px] text-zinc-500">
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
                        <p className="truncate text-[13.5px] text-zinc-400">
                          {artifact.creation_reason}
                        </p>
                      </td>
                      <td className="px-3 py-2.5">
                        <span className="font-mono text-[13px] text-zinc-500">
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
                          {artifact.action_type === "CREATED"
                            ? "criado"
                            : artifact.action_type === "DELETED"
                              ? "excluído"
                              : artifact.action_type === "MODIFIED"
                                ? "alterado"
                                : artifact.action_type}
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
                            <span className="text-[11.5px] text-zinc-500">—</span>
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

      <Modal
        open={!!selected}
        onClose={() => setSelected(null)}
        title={selected?.file_path ?? "Artefato"}
        description={
          selected ? shortPath(selected.absolute_path, 90) : undefined
        }
        size="xl"
        className="max-w-[72rem]"
      >
        {selected ? (
          <div className="space-y-3">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="flex flex-wrap items-center gap-1.5">
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
                <Badge variant="default">
                  {artifactActionLabel(selected.action_type)}
                </Badge>
                <Badge variant="default">{relativeTime(selected.created_at)}</Badge>
              </div>
              <div className="flex flex-wrap items-center gap-1.5">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void togglePin(selected)}
                  title={selected.pinned ? "Desafixar entregável" : "Fixar como entregável"}
                >
                  <Star size={13} fill={selected.pinned ? "currentColor" : "none"} />
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
                    <RotateCcw size={13} />
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
                  onClick={() => void reveal(containingFolder(selected.absolute_path))}
                >
                  <ExternalLink size={13} />
                  Abrir pasta
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void saveArtifact(selected)}
                >
                  <Download size={13} />
                  Baixar
                </Button>
              </div>
            </div>

            <div className="flex items-center gap-1 border-b border-zinc-800/70 pb-1">
              <button
                type="button"
                onClick={() => setTab("content")}
                className={cn(
                  "inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-[13.5px] transition-colors",
                  tab === "content"
                    ? "bg-zinc-800 text-zinc-100"
                    : "text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200",
                )}
              >
                <FileText size={14} />
                Conteúdo
              </button>
              <button
                type="button"
                onClick={() => setTab("diff")}
                className={cn(
                  "inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-[13.5px] transition-colors",
                  tab === "diff"
                    ? "bg-zinc-800 text-zinc-100"
                    : "text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200",
                )}
              >
                <GitCompare size={14} />
                Diff
              </button>
            </div>

            <div className="flex h-[min(64dvh,52rem)] min-h-[360px] flex-col overflow-hidden rounded-xl border border-zinc-800/70 bg-zinc-950/30">
              {tab === "content" ? (
                contentLoading ? (
                  <p className="p-4 text-[13.5px] text-zinc-500">
                    Carregando visualizador…
                  </p>
                ) : contentError ? (
                  <div className="space-y-2 p-4">
                    {contentError.includes("outside the workspace") ? (
                      <>
                        <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[13.5px] text-amber-200">
                          Este artefato está fora do workspace. Por segurança, a
                          prévia interna fica desativada para arquivos que saem do
                          diretório do workspace.
                        </p>
                        <p className="truncate font-mono text-[12px] text-zinc-500">
                          {selected.absolute_path}
                        </p>
                      </>
                    ) : (
                      <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13.5px] text-red-300">
                        Não foi possível carregar a prévia: {contentError}
                      </p>
                    )}
                    <div className="flex flex-wrap gap-2">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => void reveal(selected.absolute_path)}
                        title="Abrir o arquivo no sistema"
                      >
                        Abrir arquivo externamente
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() =>
                          void navigator.clipboard.writeText(selected.absolute_path)
                        }
                      >
                        Copiar caminho
                      </Button>
                    </div>
                  </div>
                ) : content ? (
                  <LazyFileViewer
                    embedded
                    name={selected.file_path}
                    path={content.path}
                    content={content.content}
                    binary={content.binary}
                    mime={content.mime}
                    truncated={content.truncated}
                    size={content.size}
                    rawUrl={`/api/artifacts/${selected.id}/raw`}
                    onOpenExternal={() => void reveal(selected.absolute_path)}
                    onOpenFolder={() =>
                      void reveal(containingFolder(selected.absolute_path))
                    }
                    onDownload={() => void saveArtifact(selected)}
                    archive={archive}
                    archiveLoading={archiveLoading}
                    archiveError={archiveError}
                  />
                ) : (
                  <p className="p-4 text-[13.5px] text-zinc-500">
                    Sem prévia para este artefato.
                  </p>
                )
              ) : (
                <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
                  <p className="mx-4 mt-3 text-[13.5px] leading-relaxed text-zinc-400">
                    {selected.creation_reason}
                  </p>
                  {selected.diff_patch ? (
                    <DiffView patch={selected.diff_patch} />
                  ) : (
                    <EmptyState
                      icon={GitCompare}
                      title="Sem diff registrado"
                      description="Este artefato não tem patch para comparar."
                      compact
                      className="m-4 border-0 bg-transparent"
                    />
                  )}
                </div>
              )}
            </div>
          </div>
        ) : null}
      </Modal>
    </PageBody>
  );
}
