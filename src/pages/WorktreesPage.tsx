import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowLeftRight,
  GitBranch,
  Loader2,
  RefreshCw,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/modal";
import { SectionTabs } from "@/components/ui/section-tabs";
import { DiffView } from "@/components/viewer/DiffView";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import type { AbDetail, AbGroup, WorktreeEntry } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { useUiStore } from "@/store/ui-store";
import { WorktreeTable } from "./worktrees/WorktreeTable";
import { AbCreateCard } from "./worktrees/AbCreateCard";
import { AbGroupsCard } from "./worktrees/AbGroupsCard";
import { AbDetailCard } from "./worktrees/AbDetailCard";
import type { AbForm } from "./worktrees/worktrees-shared";

function WorktreesContent({ embedded = false }: { embedded?: boolean }) {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const setWorkspaceDialogOpen = useUiStore((state) => state.setWorkspaceDialogOpen);
  const [tab, setTab] = useState("isolated");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [worktrees, setWorktrees] = useState<WorktreeEntry[]>([]);
  const [groups, setGroups] = useState<AbGroup[]>([]);
  const [detail, setDetail] = useState<AbDetail | null>(null);
  const [diff, setDiff] = useState<{ title: string; patch: string } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [form, setForm] = useState<AbForm>({
    title: "",
    prompt: "",
    agentA: "build",
    agentB: "build",
    modelA: "",
    modelB: "",
  });

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setWorktrees([]);
      setGroups([]);
      return;
    }
    setLoading(true);
    try {
      const [trees, abGroups] = await Promise.all([
        apiGet<WorktreeEntry[]>(
          `/api/worktrees?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        ),
        apiGet<AbGroup[]>(
          `/api/runs/ab?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        ),
      ]);
      setWorktrees(trees);
      setGroups(abGroups);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  const openDetail = useCallback(async (id: string) => {
    setBusy(`detail:${id}`);
    try {
      setDetail(await apiGet<AbDetail>(`/api/runs/ab/${encodeURIComponent(id)}`));
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível abrir o A/B",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }, []);

  async function showTreeDiff(entry: WorktreeEntry) {
    setBusy(`diff:${entry.id}`);
    try {
      const value = await apiGet<{ diff: string }>(
        `/api/worktrees/${encodeURIComponent(entry.id)}/diff`,
      );
      setDiff({
        title: entry.task_title ?? `worktree ${entry.id.slice(0, 8)}`,
        patch: value.diff,
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar o diff",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function mergeTree(entry: WorktreeEntry) {
    const ok = await confirmAction({
      title: "Mesclar worktree?",
      description:
        "As alterações isoladas serão aplicadas ao workspace principal. A aprovação é explícita e registrada.",
      confirmLabel: "Mesclar",
    });
    if (!ok) return;
    setBusy(`merge:${entry.id}`);
    try {
      await apiPost(`/api/worktrees/${encodeURIComponent(entry.id)}/merge`, {
        approved: true,
      });
      toast({ tone: "success", title: "Worktree mesclada" });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Merge falhou",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function discardTree(entry: WorktreeEntry) {
    const ok = await confirmAction({
      title: "Descartar worktree?",
      description:
        "O diretório isolado e a branch temporária serão removidos. As alterações não são aplicadas ao workspace.",
      confirmLabel: "Descartar",
      tone: "destructive",
    });
    if (!ok) return;
    setBusy(`discard:${entry.id}`);
    try {
      await apiDelete(`/api/worktrees/${encodeURIComponent(entry.id)}`);
      toast({ tone: "success", title: "Worktree descartada" });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível descartar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function createAb() {
    if (!activeWorkspaceId) return;
    if (!form.prompt.trim()) {
      toast({ tone: "warning", title: "Informe o prompt do A/B" });
      return;
    }
    if (!form.modelA.trim() || !form.modelB.trim()) {
      toast({ tone: "warning", title: "Informe os dois modelos" });
      return;
    }
    setCreating(true);
    try {
      const created = await apiPost<AbDetail>("/api/runs/ab", {
        workspace_id: activeWorkspaceId,
        title: form.title.trim() || undefined,
        prompt: form.prompt.trim(),
        agent_a: form.agentA,
        agent_b: form.agentB,
        model_a: form.modelA.trim(),
        model_b: form.modelB.trim(),
        effort: "medium",
      });
      setDetail(created);
      setForm({ ...form, prompt: "", title: "" });
      toast({
        tone: "success",
        title: "A/B criado",
        description: "Os dois runs foram enfileirados em worktrees isoladas.",
      });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível criar o A/B",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setCreating(false);
    }
  }

  async function choose(slot: string) {
    if (!detail) return;
    const ok = await confirmAction({
      title: `Escolher o slot ${slot.toUpperCase()}?`,
      description:
        "O vencedor será mesclado no workspace principal e o worktree perdedor será descartado. Esta é a aprovação HITL do A/B.",
      confirmLabel: "Mesclar vencedor",
    });
    if (!ok) return;
    setBusy(`choose:${slot}`);
    try {
      const chosen = await apiPost<AbDetail>(
        `/api/runs/ab/${encodeURIComponent(detail.id)}/choose`,
        { slot, approved: true },
      );
      setDetail(chosen);
      toast({ tone: "success", title: `Slot ${slot.toUpperCase()} mesclado` });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Escolha falhou",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  const activeTrees = useMemo(
    () => worktrees.filter((entry) => entry.status === "ACTIVE"),
    [worktrees],
  );

  if (!activeWorkspaceId) {
    const empty = (
      <EmptyState
        icon={GitBranch}
        title="Nenhum workspace ativo"
        description="Selecione um workspace para ver execuções isoladas e experimentos A/B."
        action={
          <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
            Criar workspace
          </Button>
        }
      />
    );
    return embedded ? (
      empty
    ) : (
      <PageBody>
        <PageHeader title="Worktrees" />
        {empty}
      </PageBody>
    );
  }

  const body = (
    <>
      <div className={embedded ? "hidden" : undefined}>
      <PageHeader
        title="Worktrees"
        description="Execuções isoladas, diffs e comparações A/B antes de tocar no workspace principal."
        actions={
          <Button
            variant="outline"
            size="sm"
            onClick={() => void load()}
            disabled={loading}
          >
            {loading ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />}
            Atualizar
          </Button>
        }
      />
      </div>

      <SectionTabs
        value={tab}
        onChange={setTab}
        items={[
          { id: "isolated", label: "Isolados", icon: GitBranch, badge: activeTrees.length },
          { id: "ab", label: "A/B", icon: ArrowLeftRight, badge: groups.length },
        ]}
      />

      {error ? (
        <Card>
          <CardContent className="text-[13.5px] text-red-300">{error}</CardContent>
        </Card>
      ) : null}

      {tab === "isolated" ? (
        <WorktreeTable
          loading={loading}
          worktrees={worktrees}
          busy={busy}
          onDiff={(entry) => void showTreeDiff(entry)}
          onMerge={(entry) => void mergeTree(entry)}
          onDiscard={(entry) => void discardTree(entry)}
        />
      ) : (
        <div className="grid gap-5 xl:grid-cols-[0.9fr_1.5fr]">
          <AbCreateCard
            form={form}
            setForm={setForm}
            creating={creating}
            workspaceId={activeWorkspaceId}
            onCreate={() => void createAb()}
          />

          <div className="space-y-5">
            <AbGroupsCard
              groups={groups}
              busy={busy}
              onOpenDetail={(id) => void openDetail(id)}
            />

            {detail ? (
              <AbDetailCard
                detail={detail}
                busy={busy}
                onChoose={(slot) => void choose(slot)}
                onShowDiff={(title, patch) => setDiff({ title, patch })}
              />
            ) : null}
          </div>
        </div>
      )}

      <Modal
        open={diff !== null}
        onClose={() => setDiff(null)}
        title={diff?.title ?? "Diff"}
        size="xl"
        initialFocus="dialog"
      >
        <div className="max-h-[70vh] overflow-auto rounded-lg border border-zinc-800/70">
          <DiffView patch={diff?.patch ?? ""} />
        </div>
      </Modal>
    </>
  );
  return embedded ? (
    <div className="space-y-4">{body}</div>
  ) : (
    <PageBody>{body}</PageBody>
  );
}

export function WorktreesPage() {
  return <WorktreesContent />;
}

export function WorktreesTabContent() {
  return <WorktreesContent embedded />;
}
