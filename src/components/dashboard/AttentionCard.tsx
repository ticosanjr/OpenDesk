import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import {
  AlertTriangle,
  Ban,
  Clock3,
  ExternalLink,
  FileCheck2,
  RefreshCw,
  ShieldAlert,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import { Link } from "react-router-dom";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { IconLabel } from "@/components/ui/icon-label";
import { Tooltip } from "@/components/ui/tooltip";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import {
  hitlConsequences,
  hitlTypeLabel,
  parseHitlOptions,
} from "@/lib/hitl";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";

interface PermissionRequest {
  id: string;
  request_id: string;
  task_id?: string | null;
  run_id?: string | null;
  permission_action: string;
  resources?: string | null;
  status: string;
  created_at: string;
}

interface HitlRequest {
  token: string;
  recipe_run_id: string;
  workspace_id: string;
  checkpoint_id: string;
  interaction_type: string;
  title?: string | null;
  question_markdown?: string | null;
  context_json?: string | null;
  options_json?: string | null;
  on_timeout?: string | null;
  expires_at?: string | null;
  status: string;
  created_at: string;
}

interface Proposal {
  id: string;
  kind: string;
  action: string;
  title: string;
  description?: string | null;
  status: string;
  requested_by?: string | null;
  apply_error?: string | null;
}

interface Attention {
  requests: PermissionRequest[];
  hitl: HitlRequest[];
  proposals: Proposal[];
  awaiting_approval: Array<{
    id: string;
    title: string;
    phase_status?: string | null;
    plan_excerpt?: string | null;
    version?: string | null;
  }>;
  blocked: Array<{
    id: string;
    title: string;
    reason?: string | null;
    version?: string | null;
  }>;
  failed_runs: Array<{
    id: string;
    task_id?: string | null;
    title?: string | null;
    status: string;
    error?: string | null;
    version?: string | null;
  }>;
}

const EMPTY: Attention = {
  requests: [],
  hitl: [],
  proposals: [],
  awaiting_approval: [],
  blocked: [],
  failed_runs: [],
};

export function AttentionCard() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastPermissionEvent = useAppStore((state) => state.lastPermissionEvent);
  const lastRecipeEvent = useAppStore((state) => state.lastRecipeEvent);
  const lastProposalEvent = useAppStore((state) => state.lastProposalEvent);
  const [data, setData] = useState<Attention | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [feedbackToken, setFeedbackToken] = useState<string | null>(null);
  const [feedbackText, setFeedbackText] = useState("");

  const load = useCallback(async () => {
    if (!workspaceId) {
      setData(null);
      return;
    }
    try {
      const attention = await apiGet<Attention>(
        `/api/attention?workspace_id=${encodeURIComponent(workspaceId)}`,
      );
      setData({ ...EMPTY, ...attention });
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
  ]);

  if (!data) return null;

  const total =
    data.requests.length +
    data.hitl.length +
    data.proposals.length +
    data.awaiting_approval.length +
    data.blocked.length +
    data.failed_runs.length;

  if (total === 0) return null;

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
    <Card data-testid="attention-card" className="border-amber-500/30 bg-amber-500/[0.04]">
      <CardContent className="space-y-3 p-3.5">
        <div className="flex flex-wrap items-center gap-2">
          <AlertTriangle size={15} className="shrink-0 text-amber-400" />
          <p className="text-[13px] font-semibold text-amber-100">
            Precisa da sua atenção
          </p>
          <CountPill value={total} tone="yellow" />
          <Button
            variant="ghost"
            size="sm"
            className="ml-auto h-7"
            onClick={() => void load()}
            disabled={busy === "refresh"}
          >
            <IconLabel icon={<RefreshCw size={12} />}>atualizar</IconLabel>
          </Button>
        </div>

        <div className="scroll-area max-h-[420px] space-y-3 pr-0.5">
          {data.proposals.length > 0 ? (
            <Section title="Propostas de configuração" count={data.proposals.length} tone="violet">
              {data.proposals.map((proposal) => (
                <Row
                  key={`proposal-${proposal.id}`}
                  tone="violet"
                  icon={<Sparkles size={13} />}
                  title={proposal.title}
                  meta={`${proposal.kind} · ${proposal.action}${
                    proposal.requested_by ? ` · @${proposal.requested_by}` : ""
                  }`}
                  detail={proposal.apply_error ?? proposal.description}
                  actions={
                    <>
                      <Button
                        size="sm"
                        className="h-7"
                        disabled={busy === `proposal-${proposal.id}`}
                        onClick={() => resolveProposal(proposal, true)}
                      >
                        Aprovar
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-7 border-red-500/30 text-red-300"
                        disabled={busy === `proposal-${proposal.id}`}
                        onClick={() => resolveProposal(proposal, false)}
                      >
                        Negar
                      </Button>
                    </>
                  }
                />
              ))}
            </Section>
          ) : null}

          {data.hitl.length > 0 ? (
            <Section title="Checkpoints de receita" count={data.hitl.length} tone="amber">
              <div className="space-y-2">
                {data.hitl.map((request) => (
                  <HitlCard
                    key={`hitl-${request.token}`}
                    request={request}
                    busy={busy === `hitl-${request.token}`}
                    feedbackOpen={feedbackToken === request.token}
                    feedbackText={feedbackText}
                    onFeedbackText={setFeedbackText}
                    onToggleFeedback={() => {
                      setFeedbackToken((current) =>
                        current === request.token ? null : request.token,
                      );
                      setFeedbackText("");
                    }}
                    onApprove={() => resolveHitl(request, "APPROVED")}
                    onReject={() => resolveHitl(request, "REJECTED")}
                    onChoose={(index, value) =>
                      resolveHitl(request, "CHOICE", {
                        text: value,
                        optionIndex: index,
                      })
                    }
                    onFeedback={() => {
                      const text = feedbackText.trim();
                      if (text) {
                        resolveHitl(request, "FEEDBACK", { text });
                      }
                    }}
                  />
                ))}
              </div>
            </Section>
          ) : null}

          {data.requests.length > 0 ? (
            <Section title="Permissões" count={data.requests.length} tone="amber">
              {data.requests.map((request) => (
                <Row
                  key={`permission-${request.request_id}`}
                  tone="amber"
                  icon={<ShieldAlert size={13} />}
                  title={
                    <span>
                      <span className="font-mono text-[10px] uppercase text-amber-300">
                        {request.permission_action}
                      </span>{" "}
                      {request.resources || "ação do agente"}
                    </span>
                  }
                  meta={request.task_id ? "tarefa vinculada" : "sem tarefa"}
                  detail="O agente pediu esta permissão para continuar. Permitir uma vez vale só para esta ação; permitir nesta tarefa cobre as próximas ações iguais; negar bloqueia o run."
                  actions={
                    <>
                      <Button
                        size="sm"
                        className="h-7"
                        title="Liberar apenas esta solicitação"
                        disabled={busy === `permission-${request.request_id}`}
                        onClick={() => decide(request, "once")}
                      >
                        Permitir uma vez
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-7"
                        title="Liberar ações iguais nesta tarefa"
                        disabled={busy === `permission-${request.request_id}`}
                        onClick={() => decide(request, "always")}
                      >
                        Nesta tarefa
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-7 border-red-500/30 text-red-300"
                        title="Bloquear a ação e interromper o run"
                        disabled={busy === `permission-${request.request_id}`}
                        onClick={() => decide(request, "reject")}
                      >
                        Negar
                      </Button>
                      {request.task_id ? (
                        <Link
                          to={links.task(request.task_id)}
                          className="inline-flex items-center gap-1 text-[11px] text-zinc-400 hover:text-zinc-200"
                        >
                          <ExternalLink size={10} />
                          abrir
                        </Link>
                      ) : null}
                    </>
                  }
                />
              ))}
            </Section>
          ) : null}

          {data.awaiting_approval.length > 0 ? (
            <Section
              title="Planos aguardando aprovação"
              count={data.awaiting_approval.length}
              tone="amber"
            >
              {data.awaiting_approval.map((task) => (
                <Row
                  key={`approval-${task.id}`}
                  tone="amber"
                  icon={<FileCheck2 size={13} />}
                  title={task.title}
                  meta="plano pronto para revisão"
                  detail={task.plan_excerpt}
                  actions={
                    <>
                      <Button
                        size="sm"
                        className="h-7"
                        title="Aprovar o plano e iniciar a fase de Build"
                        disabled={busy === `approve-${task.id}`}
                        onClick={() => approve(task.id)}
                      >
                        Aprovar e iniciar Build
                      </Button>
                      <Link
                        to={links.task(task.id)}
                        className="inline-flex items-center gap-1 text-[11px] text-zinc-400 hover:text-zinc-200"
                      >
                        <ExternalLink size={10} />
                        abrir
                      </Link>
                      <DismissButton
                        label="Dispensar alerta de aprovação"
                        onClick={() => dismiss("APPROVAL", task.id, task.version)}
                      />
                    </>
                  }
                />
              ))}
            </Section>
          ) : null}

          {data.blocked.length > 0 ? (
            <Section title="Tarefas bloqueadas" count={data.blocked.length} tone="red">
              {data.blocked.map((task) => (
                <Row
                  key={`blocked-${task.id}`}
                  tone="red"
                  icon={<Ban size={13} />}
                  title={task.title}
                  meta="bloqueada"
                  detail={task.reason}
                  actions={
                    <>
                      <Link
                        to={links.task(task.id)}
                        className="inline-flex items-center gap-1 text-[11px] text-zinc-400 hover:text-zinc-200"
                      >
                        <ExternalLink size={10} />
                        resolver
                      </Link>
                      <Tooltip content="Remover tarefa">
                        <Button
                          size="sm"
                          variant="outline"
                          className="h-7 border-red-500/30 px-2 text-red-300"
                          aria-label="Remover tarefa"
                          disabled={busy === `remove-${task.id}`}
                          onClick={() => void removeTask(task.id, task.title)}
                        >
                          <Trash2 size={12} />
                        </Button>
                      </Tooltip>
                      <DismissButton
                        label="Dispensar alerta de bloqueio"
                        onClick={() => dismiss("BLOCKED", task.id, task.version)}
                      />
                    </>
                  }
                />
              ))}
            </Section>
          ) : null}

          {data.failed_runs.length > 0 ? (
            <Section title="Falhas nas últimas 24h" count={data.failed_runs.length} tone="red">
              {data.failed_runs.map((run) => (
                <Row
                  key={`failed-${run.id}`}
                  tone="red"
                  icon={<AlertTriangle size={13} />}
                  title={run.title ?? "Execução"}
                  meta="falhou"
                  detail={run.error}
                  actions={
                    <>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-7"
                        disabled={busy === `retry-${run.id}`}
                        onClick={() => retryOther(run.id)}
                      >
                        <IconLabel icon={<RefreshCw size={11} />}>
                          Retry com outro modelo
                        </IconLabel>
                      </Button>
                      <Link
                        to={links.run(run.id)}
                        className="inline-flex items-center gap-1 text-[11px] text-zinc-400 hover:text-zinc-200"
                      >
                        <ExternalLink size={10} />
                        logs
                      </Link>
                      {run.task_id ? (
                        <Tooltip content="Remover tarefa">
                          <Button
                            size="sm"
                            variant="outline"
                            className="h-7 border-red-500/30 px-2 text-red-300"
                            aria-label="Remover tarefa"
                            disabled={busy === `remove-${run.task_id}`}
                            onClick={() =>
                              void removeTask(run.task_id as string, run.title ?? "Execução")
                            }
                          >
                            <Trash2 size={12} />
                          </Button>
                        </Tooltip>
                      ) : null}
                      <DismissButton
                        label="Dispensar alerta de falha"
                        onClick={() => dismiss("FAILED_RUN", run.id, run.version)}
                      />
                    </>
                  }
                />
              ))}
            </Section>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}

function Section({
  title,
  count,
  tone,
  children,
}: {
  title: string;
  count: number;
  tone: "amber" | "red" | "violet";
  children: ReactNode;
}) {
  return (
    <section className="space-y-1.5">
      <div className="flex items-center gap-2 px-0.5">
        <span className="text-[10px] font-semibold uppercase tracking-wide text-zinc-500">
          {title}
        </span>
        <CountPill value={count} tone={tone} />
      </div>
      <div className="space-y-1.5">{children}</div>
    </section>
  );
}

function HitlCard({
  request,
  busy,
  feedbackOpen,
  feedbackText,
  onFeedbackText,
  onToggleFeedback,
  onApprove,
  onReject,
  onChoose,
  onFeedback,
}: {
  request: HitlRequest;
  busy: boolean;
  feedbackOpen: boolean;
  feedbackText: string;
  onFeedbackText: (value: string) => void;
  onToggleFeedback: () => void;
  onApprove: () => void;
  onReject: () => void;
  onChoose: (index: number, value: string) => void;
  onFeedback: () => void;
}) {
  const consequences = hitlConsequences(request);
  const options = parseHitlOptions(request.options_json);
  const isChoice = request.interaction_type === "MULTI_CHOICE";
  const isAck = request.interaction_type === "ACKNOWLEDGE";
  const supportsFeedback =
    request.interaction_type === "APPROVAL_WITH_FEEDBACK" ||
    request.interaction_type === "ARTIFACT_REVIEW";
  const expiry = request.expires_at
    ? new Date(request.expires_at).toLocaleString("pt-BR")
    : null;
  return (
    <div className="space-y-2 rounded-lg border border-amber-500/25 bg-zinc-950/50 p-3">
      <div className="flex flex-wrap items-start gap-2">
        <Clock3 size={14} className="mt-0.5 shrink-0 text-amber-300" />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <p className="text-[12px] font-medium text-amber-100">
              {request.title || "Receita aguardando decisão"}
            </p>
            <span className="rounded border border-amber-500/25 bg-amber-500/10 px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-wide text-amber-300">
              {hitlTypeLabel(request.interaction_type)}
            </span>
            <span className="font-mono text-[9px] text-zinc-600">
              {request.checkpoint_id}
            </span>
          </div>
          <p className="mt-0.5 text-[9px] text-zinc-600">
            {expiry ? `expira em ${expiry}` : "sem prazo informado"}
          </p>
        </div>
        <Link
          to={links.run(request.recipe_run_id)}
          className="inline-flex items-center gap-1 text-[10px] text-zinc-500 hover:text-zinc-300"
        >
          <ExternalLink size={10} />
          ver run
        </Link>
      </div>

      <p className="whitespace-pre-line rounded-lg border border-zinc-800/70 bg-zinc-900/40 px-3 py-2 text-[11px] leading-relaxed text-zinc-200">
        {request.question_markdown?.trim() ||
          "Este checkpoint precisa da sua decisão para o fluxo continuar."}
      </p>

      <div className="grid gap-1.5 sm:grid-cols-3">
        <div className="rounded-md border border-emerald-500/20 bg-emerald-500/[0.06] px-2 py-1.5">
          <p className="text-[9px] font-medium uppercase tracking-wide text-emerald-300">
            Se aprovar
          </p>
          <p className="mt-0.5 text-[9px] leading-snug text-zinc-400">
            {consequences.approve}
          </p>
        </div>
        <div className="rounded-md border border-red-500/20 bg-red-500/[0.06] px-2 py-1.5">
          <p className="text-[9px] font-medium uppercase tracking-wide text-red-300">
            Se rejeitar
          </p>
          <p className="mt-0.5 text-[9px] leading-snug text-zinc-400">
            {consequences.reject}
          </p>
        </div>
        <div className="rounded-md border border-amber-500/20 bg-amber-500/[0.06] px-2 py-1.5">
          <p className="text-[9px] font-medium uppercase tracking-wide text-amber-300">
            Se expirar
          </p>
          <p className="mt-0.5 text-[9px] leading-snug text-zinc-400">
            {consequences.timeout}
          </p>
        </div>
      </div>

      {isChoice && options.length > 0 ? (
        <div className="flex flex-wrap gap-1.5">
          {options.map((option, index) => (
            <Button
              key={`${request.token}-${index}`}
              size="sm"
              variant="outline"
              className="h-7"
              disabled={busy}
              onClick={() => onChoose(index, option)}
            >
              {index + 1}. {option}
            </Button>
          ))}
        </div>
      ) : null}

      {feedbackOpen ? (
        <div className="space-y-1.5">
          <textarea
            value={feedbackText}
            onChange={(event) => onFeedbackText(event.target.value)}
            rows={3}
            placeholder="Explique o que deve ser ajustado antes de refazer esta etapa."
            className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-950/60 px-2.5 py-2 text-[11px] text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-amber-500/50"
            aria-label="Ajustes solicitados"
          />
          <div className="flex justify-end gap-1.5">
            <Button size="sm" variant="ghost" className="h-7" onClick={onToggleFeedback}>
              Cancelar
            </Button>
            <Button
              size="sm"
              className="h-7"
              disabled={busy || !feedbackText.trim()}
              onClick={onFeedback}
            >
              Enviar ajustes
            </Button>
          </div>
        </div>
      ) : null}

      <div className="flex flex-wrap items-center justify-end gap-1.5">
        {!isChoice ? (
          <Button size="sm" className="h-7" disabled={busy} onClick={onApprove}>
            {isAck ? "Entendi, continuar" : "Aprovar e continuar"}
          </Button>
        ) : null}
        {supportsFeedback && !feedbackOpen ? (
          <Button
            size="sm"
            variant="outline"
            className="h-7"
            disabled={busy}
            onClick={onToggleFeedback}
          >
            Pedir ajustes
          </Button>
        ) : null}
        {!isAck ? (
          <Button
            size="sm"
            variant="outline"
            className="h-7 border-red-500/30 text-red-300"
            disabled={busy}
            onClick={onReject}
          >
            Rejeitar
          </Button>
        ) : null}
      </div>
    </div>
  );
}

function Row({
  icon,
  title,
  meta,
  detail,
  actions,
  tone,
}: {
  icon: ReactNode;
  title: ReactNode;
  meta?: string;
  detail?: string | null;
  actions?: ReactNode;
  tone: "amber" | "red" | "violet";
}) {
  const border =
    tone === "red"
      ? "border-red-500/20"
      : tone === "violet"
        ? "border-violet-500/20"
        : "border-amber-500/20";
  const iconTone =
    tone === "red"
      ? "text-red-300"
      : tone === "violet"
        ? "text-violet-300"
        : "text-amber-300";
  return (
    <div
      data-attention-row
      className={cn(
        "grid min-h-[82px] grid-cols-[auto_minmax(0,1fr)_auto] items-start gap-x-2.5 gap-y-1 rounded-lg border bg-zinc-950/40 px-2.5 py-2 sm:h-[82px] sm:grid-cols-[auto_minmax(0,1fr)_250px] sm:items-stretch sm:overflow-hidden",
        border,
      )}
    >
      <span className={cn("mt-0.5 shrink-0", iconTone)}>{icon}</span>
      <div className="min-h-0 min-w-0 sm:overflow-y-auto">
        <p className="line-clamp-1 text-[12px] text-zinc-200">{title}</p>
        {meta ? (
          <p className="truncate font-mono text-[10px] text-zinc-500">{meta}</p>
        ) : null}
        {detail ? (
          <p className="mt-0.5 line-clamp-2 min-h-[26px] break-words text-[10px] leading-snug text-red-200/70">
            {detail}
          </p>
        ) : null}
      </div>
      {actions ? (
        <div className="col-span-3 flex min-h-0 flex-wrap items-center justify-end gap-1.5 sm:col-span-1 sm:w-[250px] sm:max-w-[250px] sm:overflow-y-auto">
          {actions}
        </div>
      ) : null}
    </div>
  );
}

function DismissButton({
  label,
  onClick,
}: {
  label: string;
  onClick: () => void;
}) {
  return (
    <Tooltip content={label}>
      <button
        type="button"
        aria-label={label}
        onClick={onClick}
        className="rounded p-1 text-zinc-600 transition-colors hover:bg-zinc-800/60 hover:text-zinc-200"
      >
        <X size={12} />
      </button>
    </Tooltip>
  );
}
