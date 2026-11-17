import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowLeftRight,
  GitBranch,
  GitMerge,
  Loader2,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { Link } from "react-router-dom";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { Field, Input } from "@/components/ui/field";
import { LoadingState } from "@/components/ui/loading-state";
import { Modal } from "@/components/ui/modal";
import { SectionTabs } from "@/components/ui/section-tabs";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { DiffView } from "@/components/viewer/DiffView";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { formatDuration, formatNumber, formatUsd, relativeTime } from "@/lib/format";
import type { AbDetail, AbGroup, WorktreeEntry } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { useUiStore } from "@/store/ui-store";

function worktreeTone(status: string) {
  if (status === "MERGED") return "green" as const;
  if (status === "REMOVED") return "red" as const;
  return "blue" as const;
}

function abTone(status: string) {
  if (status === "DECIDED") return "green" as const;
  if (status === "CANCELLED") return "red" as const;
  return "blue" as const;
}

export function WorktreesPage() {
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
  const [form, setForm] = useState({
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
    return (
      <PageBody>
        <PageHeader title="Worktrees" />
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
      </PageBody>
    );
  }

  return (
    <PageBody>
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
        <Card>
          <CardHeader>
            <CardTitle>Execuções isoladas</CardTitle>
            <p className="text-[13px] text-zinc-500">
              Cada worktree pertence a um run. Merge exige aprovação; descartar remove
              a cópia isolada.
            </p>
          </CardHeader>
          <CardContent className="p-0">
            {loading && worktrees.length === 0 ? (
              <LoadingState label="Carregando worktrees…" />
            ) : worktrees.length === 0 ? (
              <EmptyState
                icon={GitBranch}
                title="Nenhuma worktree"
                description="Ative runs isolados em Configurações ou crie um A/B para gerar duas worktrees."
              />
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full min-w-[820px] border-collapse text-left text-[13px]">
                  <thead className="border-b border-zinc-800/70 text-[12px] uppercase tracking-wide text-zinc-500">
                    <tr>
                      <th className="px-4 py-2 font-medium">Execução</th>
                      <th className="px-4 py-2 font-medium">Isolamento</th>
                      <th className="px-4 py-2 font-medium">Diff</th>
                      <th className="px-4 py-2 font-medium">Status</th>
                      <th className="px-4 py-2 font-medium">Criada</th>
                      <th className="px-4 py-2 text-right font-medium">Ações</th>
                    </tr>
                  </thead>
                  <tbody>
                    {worktrees.map((entry) => (
                      <tr
                        key={entry.id}
                        className="border-b border-zinc-800/50 align-top last:border-0"
                      >
                        <td className="px-4 py-3">
                          <div className="font-medium text-zinc-200">
                            {entry.task_title ?? "Execução sem tarefa"}
                          </div>
                          {entry.run_id ? (
                            <Link
                              to={`/runs/${encodeURIComponent(entry.run_id)}`}
                              className="font-mono text-[12px] text-blue-300 hover:underline"
                            >
                              run {entry.run_id.slice(0, 12)}…
                            </Link>
                          ) : null}
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-2 text-zinc-300">
                            <GitBranch size={13} className="text-zinc-500" />
                            {entry.mode}
                          </div>
                          {entry.branch ? (
                            <div className="mt-1 font-mono text-[12px] text-zinc-500">
                              {entry.branch}
                            </div>
                          ) : null}
                        </td>
                        <td className="px-4 py-3 font-mono text-[12px]">
                          <span className="text-zinc-400">{entry.diff_stat.files} arq.</span>{" "}
                          <span className="text-emerald-300">
                            +{entry.diff_stat.insertions}
                          </span>{" "}
                          <span className="text-red-300">
                            -{entry.diff_stat.deletions}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <Badge variant={worktreeTone(entry.status)}>{entry.status}</Badge>
                        </td>
                        <td className="px-4 py-3 text-zinc-500">
                          {relativeTime(entry.created_at)}
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex justify-end gap-1.5">
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => void showTreeDiff(entry)}
                              disabled={busy === `diff:${entry.id}`}
                            >
                              {busy === `diff:${entry.id}` ? (
                                <Loader2 size={12} className="animate-spin" />
                              ) : null}
                              Diff
                            </Button>
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => void mergeTree(entry)}
                              disabled={entry.status !== "ACTIVE" || busy === `merge:${entry.id}`}
                            >
                              <GitMerge size={12} />
                              Mesclar
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="text-red-300 hover:text-red-200"
                              onClick={() => void discardTree(entry)}
                              disabled={entry.status !== "ACTIVE" || busy === `discard:${entry.id}`}
                            >
                              <Trash2 size={12} />
                              Descartar
                            </Button>
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-5 xl:grid-cols-[0.9fr_1.5fr]">
          <Card>
            <CardHeader>
              <CardTitle>Novo experimento A/B</CardTitle>
              <p className="text-[13px] text-zinc-500">
                Mesmo prompt, dois agentes/modelos e duas worktrees. O vencedor só é
                mesclado após confirmação.
              </p>
            </CardHeader>
            <CardContent className="space-y-3">
              <Field label="Título" htmlFor="ab-title">
                <Input
                  id="ab-title"
                  value={form.title}
                  onChange={(event) => setForm({ ...form, title: event.target.value })}
                  placeholder="Ex.: refatorar parser"
                />
              </Field>
              <Field label="Prompt" htmlFor="ab-prompt">
                <textarea
                  id="ab-prompt"
                  value={form.prompt}
                  onChange={(event) => setForm({ ...form, prompt: event.target.value })}
                  placeholder="Descreva a tarefa que os dois lados vão executar…"
                  className="min-h-[96px] w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2 text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
                />
              </Field>
              <div className="grid gap-3 sm:grid-cols-2">
                <Field label="Agente A">
                  <AgentCombobox
                    value={form.agentA}
                    onChange={(value) => setForm({ ...form, agentA: value })}
                    workspaceId={activeWorkspaceId}
                    allowEmpty={false}
                  />
                </Field>
                <Field label="Agente B">
                  <AgentCombobox
                    value={form.agentB}
                    onChange={(value) => setForm({ ...form, agentB: value })}
                    workspaceId={activeWorkspaceId}
                    allowEmpty={false}
                  />
                </Field>
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <Field label="Modelo A">
                  <ModelCombobox
                    value={form.modelA || null}
                    onChange={(value) => setForm({ ...form, modelA: value ?? "" })}
                    allowRouter={false}
                    ariaLabel="Modelo A"
                  />
                </Field>
                <Field label="Modelo B">
                  <ModelCombobox
                    value={form.modelB || null}
                    onChange={(value) => setForm({ ...form, modelB: value ?? "" })}
                    allowRouter={false}
                    ariaLabel="Modelo B"
                  />
                </Field>
              </div>
              <Button
                size="sm"
                onClick={() => void createAb()}
                disabled={creating}
                className="w-full"
              >
                {creating ? <Loader2 size={13} className="animate-spin" /> : <ArrowLeftRight size={13} />}
                Criar A/B
              </Button>
            </CardContent>
          </Card>

          <div className="space-y-5">
            <Card>
              <CardHeader>
                <CardTitle>Experimentos</CardTitle>
              </CardHeader>
              <CardContent className="p-0">
                {groups.length === 0 ? (
                  <EmptyState
                    icon={ArrowLeftRight}
                    title="Nenhum A/B ainda"
                    description="Crie um experimento para comparar dois runs isolados."
                  />
                ) : (
                  <div className="divide-y divide-zinc-800/60">
                    {groups.map((group) => (
                      <button
                        key={group.id}
                        type="button"
                        onClick={() => void openDetail(group.id)}
                        className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-zinc-900/50"
                      >
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <span className="truncate text-[13.5px] font-medium text-zinc-200">
                              {group.title}
                            </span>
                            <Badge variant={abTone(group.status)}>{group.status}</Badge>
                            {group.winner_slot ? (
                              <Badge variant="green">vencedor {group.winner_slot.toUpperCase()}</Badge>
                            ) : null}
                          </div>
                          <p className="mt-0.5 truncate text-[12.5px] text-zinc-500">
                            {group.prompt}
                          </p>
                        </div>
                        {busy === `detail:${group.id}` ? (
                          <Loader2 size={14} className="animate-spin text-zinc-500" />
                        ) : null}
                      </button>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>

            {detail ? (
              <Card>
                <CardHeader className="flex-row items-start justify-between gap-3">
                  <div>
                    <CardTitle>{detail.title}</CardTitle>
                    <p className="mt-1 text-[13px] text-zinc-500">{detail.prompt}</p>
                  </div>
                  <Badge variant={abTone(detail.status)}>{detail.status}</Badge>
                </CardHeader>
                <CardContent>
                  <div className="grid gap-3 md:grid-cols-2">
                    {detail.slots.map((slot) => (
                      <div
                        key={slot.slot}
                        className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <span className="font-mono text-[13px] font-semibold text-zinc-200">
                            SLOT {slot.slot.toUpperCase()}
                          </span>
                          <Badge variant={slot.status === "SUCCESS" ? "green" : "blue"}>
                            {slot.status}
                          </Badge>
                        </div>
                        <div className="mt-2 space-y-1 text-[12.5px] text-zinc-400">
                          <p>
                            {slot.agent} · <span className="font-mono">{slot.model || "—"}</span>
                          </p>
                          <p className="font-mono">
                            {formatUsd(slot.cost_usd)} · {formatNumber(slot.tokens)} tokens ·{" "}
                            {formatDuration(slot.duration_ms)}
                          </p>
                          <p className="font-mono">
                            {slot.diff_stat.files} arq.{" "}
                            <span className="text-emerald-300">+{slot.diff_stat.insertions}</span>{" "}
                            <span className="text-red-300">-{slot.diff_stat.deletions}</span>
                          </p>
                        </div>
                        <div className="mt-3 flex flex-wrap gap-1.5">
                          {slot.diff ? (
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() =>
                                setDiff({
                                  title: `Slot ${slot.slot.toUpperCase()}`,
                                  patch: slot.diff ?? "",
                                })
                              }
                            >
                              Ver diff
                            </Button>
                          ) : null}
                          <Button
                            size="sm"
                            variant={detail.winner_slot === slot.slot ? "default" : "outline"}
                            disabled={
                              detail.status !== "OPEN" ||
                              slot.status !== "SUCCESS" ||
                              busy === `choose:${slot.slot}`
                            }
                            onClick={() => void choose(slot.slot)}
                          >
                            {busy === `choose:${slot.slot}` ? (
                              <Loader2 size={12} className="animate-spin" />
                            ) : (
                              <ShieldCheck size={12} />
                            )}
                            Escolher {slot.slot.toUpperCase()}
                          </Button>
                        </div>
                      </div>
                    ))}
                  </div>
                </CardContent>
              </Card>
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
    </PageBody>
  );
}
