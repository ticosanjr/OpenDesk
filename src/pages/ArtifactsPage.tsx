import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { Archive, RefreshCw } from "lucide-react";
import { openWorkspacePath } from "@/lib/open-path";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet, apiPost } from "@/lib/api";
import type { Artifact } from "@/lib/types";
import { saveLocalFile } from "@/lib/save-file";
import { links } from "@/lib/links";
import type { ArchiveEntry } from "@/components/viewer/FileViewer";
import { toast } from "@/store/toast-store";
import { confirmAction } from "@/store/confirm-store";
import { ArtifactDetailModal } from "@/pages/artifacts/ArtifactDetailModal";
import { ArtifactsExplorer } from "@/pages/artifacts/ArtifactsExplorer";
import type { ArtifactContent } from "@/pages/artifacts/artifact-helpers";

export function ArtifactsPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const lastArtifactEvent = useAppStore((state) => state.lastArtifactEvent);
  const revisions = useAppStore((state) => state.revisions);
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
  }, [load, lastArtifactEvent, revisions.artifacts]);

  // Deep link from the dashboard/search: /artifacts?artifact=<artifact-id>.
  // `id` is still accepted for older links. Fetch by id instead of depending
  // on the current list/filter window.
  useEffect(() => {
    const id = searchParams.get("artifact") ?? searchParams.get("id");
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
            next.delete("artifact");
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
      await openWorkspacePath(path);
    } catch (cause) {
      console.warn("OpenDesk: could not open path", cause);
    }
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

      <ArtifactsExplorer
        artifacts={artifacts}
        agentFilter={agentFilter}
        onAgentFilter={setAgentFilter}
        actionFilter={actionFilter}
        onActionFilter={setActionFilter}
        runFilter={runFilter}
        onClearRun={() =>
          setSearchParams((current) => {
            const next = new URLSearchParams(current);
            next.delete("run");
            return next;
          }, { replace: true })
        }
        limit={limit}
        onLoadMore={() => setLimit((value) => value + 200)}
        onSelect={(artifact) => {
          setSelected(artifact);
          setTab("content");
        }}
        onSave={(artifact) => void saveArtifact(artifact)}
        onTogglePin={(artifact) => void togglePin(artifact)}
      />

      <ArtifactDetailModal
        selected={selected}
        onClose={() => setSelected(null)}
        tab={tab}
        onTabChange={setTab}
        onTogglePin={(artifact) => void togglePin(artifact)}
        onRollback={(artifact) => void rollback(artifact)}
        onOpenRun={(runId) => navigate(links.run(runId))}
        onReveal={(path) => void reveal(path)}
        onSave={(artifact) => void saveArtifact(artifact)}
        content={content}
        contentLoading={contentLoading}
        contentError={contentError}
        archive={archive}
        archiveLoading={archiveLoading}
        archiveError={archiveError}
      />
    </PageBody>
  );
}
