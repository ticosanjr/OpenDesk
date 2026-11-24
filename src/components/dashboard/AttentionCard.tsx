import { useCallback, useEffect, useRef, useState } from "react";
import {
  AlertTriangle,
  ChevronDown,
  ChevronUp,
  RefreshCw,
} from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { IconLabel } from "@/components/ui/icon-label";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import { ApprovalsSection } from "@/components/dashboard/attention/ApprovalsSection";
import { BlockedSection } from "@/components/dashboard/attention/BlockedSection";
import { FailedRunsSection } from "@/components/dashboard/attention/FailedRunsSection";
import { HitlSection } from "@/components/dashboard/attention/HitlSection";
import { MissedRunsSection } from "@/components/dashboard/attention/MissedRunsSection";
import { PermissionsSection } from "@/components/dashboard/attention/PermissionsSection";
import { ProposalsSection } from "@/components/dashboard/attention/ProposalsSection";
import { Section } from "@/components/dashboard/attention/Section";
import { EMPTY_ATTENTION, type Attention, type HitlRequest, type PermissionRequest, type Proposal } from "@/components/dashboard/attention/types";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";
import { InteractionCard } from "@/components/interactions/InteractionCard";

export function AttentionCard() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastPermissionEvent = useAppStore((state) => state.lastPermissionEvent);
  const lastRecipeEvent = useAppStore((state) => state.lastRecipeEvent);
  const lastProposalEvent = useAppStore((state) => state.lastProposalEvent);
  const lastScheduleEvent = useAppStore((state) => state.lastScheduleEvent);
  const revisions = useAppStore((state) => state.revisions);
  const setAttentionCount = useAppStore((state) => state.setAttentionCount);
  const [data, setData] = useState<Attention | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [feedbackToken, setFeedbackToken] = useState<string | null>(null);
  const [feedbackText, setFeedbackText] = useState("");
  const [expanded, setExpanded] = useState(false);

  const load = useCallback(async () => {
    if (!workspaceId) {
      setData(null);
      return;
    }
    try {
      const attention = await apiGet<Attention>(
        `/api/attention?workspace_id=${encodeURIComponent(workspaceId)}`,
      );
      setData({ ...EMPTY_ATTENTION, ...attention });
    } catch {
      setData(null);
    }
  }, [workspaceId]);

  const mounted = useRef(false);
  useEffect(() => {
    // Coalesce bursts of run/task events into one refresh.
    const delay = mounted.current ? 250 : 0;
    mounted.current = true;
    const handle = window.setTimeout(() => void load(), delay);
    return () => window.clearTimeout(handle);
  }, [
    load,
    lastTaskEvent?.task_id,
    lastRunEvent?.run_id,
    lastRunEvent?.status,
    lastPermissionEvent?.request_id,
    lastRecipeEvent,
    lastProposalEvent,
    lastScheduleEvent,
    revisions.attention,
  ]);

  const total = data
    ? data.requests.length +
      data.missed_runs.length +
      data.task_interactions.length +
      data.hitl.length +
      data.proposals.length +
      data.awaiting_approval.length +
      data.blocked.length +
      data.failed_runs.length
    : 0;

  useEffect(() => {
    setAttentionCount(total);
  }, [setAttentionCount, total]);

  if (!data) return null;
  if (total === 0) return null;

  const summary = [
    { key: "proposals", label: "propostas", count: data.proposals.length },
    { key: "missed", label: "perdidas", count: data.missed_runs.length },
    { key: "questions", label: "perguntas", count: data.task_interactions.length },
    { key: "hitl", label: "checkpoints", count: data.hitl.length },
    { key: "permissions", label: "permissões", count: data.requests.length },
    { key: "approvals", label: "aprovações", count: data.awaiting_approval.length },
    { key: "blocked", label: "bloqueios", count: data.blocked.length },
    { key: "failed", label: "falhas", count: data.failed_runs.length },
  ].filter((item) => item.count > 0);

  function runMissed(id: string) {
    void run(`missed-${id}`, async () => {
      await apiPost(`/api/missed-runs/${id}/run`);
      toast({
        tone: "success",
        title: "Execução perdida enfileirada",
        description: "Ela vai rodar assim que houver um worker livre.",
      });
    });
  }

  function ignoreMissed(id: string) {
    void run(`missed-${id}`, async () => {
      await apiPost(`/api/missed-runs/${id}/ignore`);
      toast({ tone: "info", title: "Execução perdida ignorada" });
    });
  }

  function runAllMissed() {
    void run("missed-all", async () => {
      await apiPost(
        `/api/missed-runs/run-all?workspace_id=${encodeURIComponent(workspaceId ?? "")}`,
      );
      toast({ tone: "success", title: "Execuções perdidas enfileiradas" });
    });
  }

  function ignoreAllMissed() {
    void run("missed-all", async () => {
      await apiPost(
        `/api/missed-runs/ignore-all?workspace_id=${encodeURIComponent(workspaceId ?? "")}`,
      );
      toast({ tone: "info", title: "Execuções perdidas ignoradas" });
    });
  }

  async function run(key: string, action: () => Promise<void>) {
    setBusy(key);
    try {
      await action();
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível concluir a ação",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  function resolveProposal(proposal: Proposal, approved: boolean) {
    void run(`proposal-${proposal.id}`, async () => {
      const result = await apiPost<{ status: string; apply_error?: string | null }>(
        `/api/proposals/${proposal.id}/resolve`,
        { approved },
      );
      if (result.status === "FAILED") {
        toast({
          tone: "error",
          title: "Aprovação falhou ao aplicar",
          description: result.apply_error ?? "erro desconhecido",
        });
      } else {
        toast({
          tone: "success",
          title: approved ? "Alteração aplicada" : "Proposta negada",
        });
      }
    });
  }

  function resolveHitl(
    request: HitlRequest,
    resolution: string,
    options: { text?: string; optionIndex?: number } = {},
  ) {
    void run(`hitl-${request.token}`, async () => {
      await apiPost(`/api/recipes/hitl/${request.token}/resolve`, {
        resolution,
        text: options.text,
        option_index: options.optionIndex,
      });
      if (feedbackToken === request.token) {
        setFeedbackToken(null);
        setFeedbackText("");
      }
      toast({
        tone: "success",
        title:
          resolution === "APPROVED"
            ? "Checkpoint aprovado e fluxo retomado"
            : resolution === "REJECTED"
              ? "Checkpoint rejeitado"
              : resolution === "CHOICE"
                ? "Opção registrada"
                : "Ajustes enviados ao agente",
      });
    });
  }

  function decide(request: PermissionRequest, decision: string) {
    void run(`permission-${request.request_id}`, async () => {
      await apiPost(`/api/permissions/${request.request_id}/reply`, { decision });
      toast({
        tone: "success",
        title:
          decision === "reject"
            ? "Permissão negada"
            : decision === "always"
              ? "Permitido nesta tarefa"
              : "Permitido uma vez",
        href: request.task_id ? links.task(request.task_id) : undefined,
      });
    });
  }

  function retryOther(runId: string) {
    void run(`retry-${runId}`, async () => {
      const result = await apiPost<{ model: string }>(
        `/api/runs/${runId}/retry-other-model`,
        {},
      );
      toast({
        tone: "success",
        title: "Reenfileirado com outro modelo",
        description: result.model,
      });
    });
  }

  function approve(taskId: string) {
    void run(`approve-${taskId}`, async () => {
      await apiPost(`/api/tasks/${taskId}/approve-plan`, {});
      toast({ tone: "success", title: "Plano aprovado", href: links.task(taskId) });
    });
  }

  function dismiss(kind: string, refId: string, version?: string | null) {
    void run(`dismiss-${kind}-${refId}`, async () => {
      await apiPost("/api/attention/dismiss", {
        kind,
        ref_id: refId,
        version: version ?? "",
        workspace_id: workspaceId,
      });
      toast({ tone: "success", title: "Alerta dispensado" });
    });
  }

  async function removeTask(taskId: string, title: string) {
    const confirmed = await confirmAction({
      title: "Remover tarefa?",
      description: `“${title}” e seus runs serão removidos do OpenDesk. Os arquivos do workspace não são apagados.`,
      confirmLabel: "Remover tarefa",
      tone: "destructive",
    });
    if (!confirmed) return;
    void run(`remove-${taskId}`, async () => {
      await apiDelete(`/api/tasks/${taskId}`);
      toast({ tone: "success", title: "Tarefa removida" });
    });
  }

  return (
    <Card data-testid="attention-card" className="border-amber-500/25 bg-amber-500/[0.035]">
      <CardContent className="space-y-3 p-4">
        <div className="flex flex-wrap items-center gap-2">
          <span className="flex h-7 w-7 items-center justify-center rounded-md border border-amber-500/25 bg-amber-500/10 text-amber-300">
            <AlertTriangle size={15} className="shrink-0" />
          </span>
          <p className="text-[14px] font-semibold text-amber-100">
            Precisa da sua atenção
          </p>
          <CountPill value={total} tone="yellow" />
          <Button
            variant="ghost"
            size="xs"
            className="ml-auto"
            onClick={() => setExpanded((current) => !current)}
            aria-expanded={expanded}
          >
            <IconLabel
              icon={
                expanded ? <ChevronUp size={13} /> : <ChevronDown size={13} />
              }
            >
              {expanded ? "recolher" : "ver detalhes"}
            </IconLabel>
          </Button>
          <Button
            variant="ghost"
            size="xs"
            onClick={() => void load()}
            disabled={busy === "refresh"}
          >
            <IconLabel
              icon={
                <RefreshCw
                  size={13}
                  className={busy === "refresh" ? "animate-spin" : undefined}
                />
              }
            >
              atualizar
            </IconLabel>
          </Button>
        </div>

        <div className="flex flex-wrap gap-1">
          {summary.map((item) => (
            <span
              key={item.key}
              className="inline-flex items-center gap-1 rounded-md border border-amber-500/20 bg-amber-500/[0.06] px-1.5 py-0.5 text-[11.5px] text-amber-200/90"
            >
              <span className="font-mono text-[11.5px] text-amber-300">
                {item.count}
              </span>
              {item.label}
            </span>
          ))}
        </div>

        <div
          className={cn(
            "scroll-area space-y-3 pr-0.5",
            expanded ? "" : "max-h-[280px] overflow-y-auto",
          )}
        >
          <ProposalsSection
            proposals={data.proposals}
            busy={busy}
            onResolve={resolveProposal}
          />

          <MissedRunsSection
            runs={data.missed_runs}
            busy={busy}
            onRunAll={runAllMissed}
            onIgnoreAll={ignoreAllMissed}
            onRun={runMissed}
            onIgnore={ignoreMissed}
          />

          {data.task_interactions.length > 0 ? (
            <Section
              title="Perguntas do agente"
              count={data.task_interactions.length}
              tone="amber"
            >
              <div className="space-y-2">
                {data.task_interactions.map((interaction) => (
                  <InteractionCard
                    key={`interaction-${interaction.id}`}
                    interaction={interaction}
                    onResolved={() => void load()}
                  />
                ))}
              </div>
            </Section>
          ) : null}

          <HitlSection
            requests={data.hitl}
            busy={busy}
            feedbackToken={feedbackToken}
            feedbackText={feedbackText}
            onFeedbackText={setFeedbackText}
            onToggleFeedback={(token) => {
              setFeedbackToken((current) =>
                current === token ? null : token,
              );
              setFeedbackText("");
            }}
            onApprove={(request) => resolveHitl(request, "APPROVED")}
            onReject={(request) => resolveHitl(request, "REJECTED")}
            onChoose={(request, index, value) =>
              resolveHitl(request, "CHOICE", {
                text: value,
                optionIndex: index,
              })
            }
            onFeedback={(request) => {
              const text = feedbackText.trim();
              if (text) {
                resolveHitl(request, "FEEDBACK", { text });
              }
            }}
          />

          <PermissionsSection
            requests={data.requests}
            busy={busy}
            onDecide={decide}
          />

          <ApprovalsSection
            tasks={data.awaiting_approval}
            busy={busy}
            onApprove={approve}
            onDismiss={(taskId, version) =>
              dismiss("APPROVAL", taskId, version)
            }
          />

          <BlockedSection
            tasks={data.blocked}
            busy={busy}
            onDismiss={(taskId, version) => dismiss("BLOCKED", taskId, version)}
            onRemoveTask={(taskId, title) => void removeTask(taskId, title)}
          />

          <FailedRunsSection
            runs={data.failed_runs}
            busy={busy}
            onRetry={retryOther}
            onRemoveTask={(taskId, title) => void removeTask(taskId, title)}
            onDismiss={(runId, version) => dismiss("FAILED_RUN", runId, version)}
          />
        </div>
      </CardContent>
    </Card>
  );
}
