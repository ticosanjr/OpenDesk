import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  AlertTriangle,
  Ban,
  CheckCircle2,
  Columns3,
  FileText,
  FolderOpen,
  GitFork,
  Loader2,
  Pause,
  Pencil,
  Play,
  RotateCw,
  Square,
  XCircle,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EntityLink } from "@/components/ui/entity-link";
import { RunLogView } from "@/components/runs/RunLogView";
import { WorkflowStatusBadge } from "@/components/ui/workflow-status";
import {
  InteractionCard,
  PermissionRequestCard,
} from "@/components/interactions/InteractionCard";
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/modal";
import { apiGet, apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import { STATUS_LABEL } from "@/lib/task-ui";
import { isTaskAwaitingApproval, isTaskRunning } from "@/lib/task-state";
import type {
  Task,
  TaskDiagnostics,
  TaskInteraction,
  TaskRun,
} from "@/lib/types";
import { formatNumber, formatUsd, relativeTime } from "@/lib/format";
import type { OutboundSink } from "@/components/integrations/OutboundSinksCard";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";

export function TaskDetailsModal({
  task,
  onClose,
  onEdit,
  onApprove,
  onDispatch,
  onStatus,
  onWait,
  onFork,
  onSwarm,
  onLogs,
}: {
  task: Task | null;
  onClose: () => void;
  onEdit: (task: Task) => void;
  onApprove: (task: Task) => void;
  onDispatch: (task: Task) => void;
  onStatus: (task: Task, status: string) => void;
  onWait: (task: Task) => void;
  onFork: (task: Task) => void;
  onSwarm: (task: Task) => void;
  onLogs: (task: Task) => void;
}) {
  const [diagnostics, setDiagnostics] = useState<TaskDiagnostics | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [context, setContext] = useState<{
    session_id?: string | null;
    tokens_input: number;
    window: number;
    percent: number;
    messages: number;
  } | null>(null);
  const [compacting, setCompacting] = useState(false);
  const [sinks, setSinks] = useState<OutboundSink[]>([]);
  const [selectedSinks, setSelectedSinks] = useState<Set<string>>(new Set());
  const [savingSinks, setSavingSinks] = useState(false);
  const [interactions, setInteractions] = useState<TaskInteraction[]>([]);
  const [permissions, setPermissions] = useState<
    Array<{
      request_id: string;
      permission_action: string;
      resources?: string | null;
      task_id?: string | null;
      run_id?: string | null;
      created_at?: string;
    }>
  >([]);
  const daemon = useAppStore((state) => state.daemon);
  const navigate = useNavigate();

  const taskId = task?.id;
  const taskWorkspaceId = task?.workspace_id;

  useEffect(() => {
    if (!taskId || !taskWorkspaceId) {
      setSinks([]);
      setSelectedSinks(new Set());
      return;
    }
    let cancelled = false;
    void Promise.all([
      apiGet<{ sinks: OutboundSink[] }>(
        `/api/io/outbound?workspace_id=${encodeURIComponent(taskWorkspaceId)}`,
      ),
      apiGet<{ bindings: Array<{ sink_id: string }> }>(
        `/api/tasks/${taskId}/sinks`,
      ),
    ])
      .then(([sinkList, bindings]) => {
        if (cancelled) return;
        setSinks(sinkList.sinks);
        setSelectedSinks(new Set(bindings.bindings.map((binding) => binding.sink_id)));
      })
      .catch(() => {
        if (!cancelled) {
          setSinks([]);
          setSelectedSinks(new Set());
        }
      });
    return () => {
      cancelled = true;
    };
  }, [taskId, taskWorkspaceId]);

  async function saveSinks() {
    if (!task) return;
    setSavingSinks(true);
    try {
      await apiPost(`/api/tasks/${task.id}/sinks`, {
        sink_ids: Array.from(selectedSinks),
      });
      toast({ tone: "success", title: "Entregas automáticas atualizadas" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar as entregas",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSavingSinks(false);
    }
  }

  async function loadInteractions() {
    if (!taskId || !taskWorkspaceId) {
      setInteractions([]);
      setPermissions([]);
      return;
    }
    try {
      const [pendingInteractions, permissionResponse] = await Promise.all([
        apiGet<TaskInteraction[]>(`/api/tasks/${taskId}/interactions`),
        apiGet<{
          requests: Array<{
            request_id: string;
            permission_action: string;
            resources?: string | null;
            task_id?: string | null;
            run_id?: string | null;
          }>;
        }>(
          `/api/permissions?workspace_id=${encodeURIComponent(taskWorkspaceId)}`,
        ),
      ]);
      setInteractions(
        Array.isArray(pendingInteractions) ? pendingInteractions : [],
      );
      setPermissions(
        (Array.isArray(permissionResponse?.requests)
          ? permissionResponse.requests
          : []
        ).filter(
          (request) =>
            request.task_id === taskId ||
            (request.run_id && request.run_id === task?.last_run_id),
        ),
      );
    } catch {
      setInteractions([]);
      setPermissions([]);
    }
  }

  useEffect(() => {
    void loadInteractions();
    // taskId/taskWorkspaceId cover task changes; loadInteractions is intentionally
    // called only when the task identity changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [taskId, taskWorkspaceId, task?.last_run_id]);

  useEffect(() => {
    if (!task) {
      setContext(null);
      return;
    }
    let cancelled = false;
    apiGet<{
      session_id?: string | null;
      tokens_input: number;
      window: number;
      percent: number;
      messages: number;
    }>(`/api/tasks/${task.id}/context`)
      .then((value) => {
        if (!cancelled) setContext(value);
      })
      .catch(() => {
        if (!cancelled) setContext(null);
      });
    return () => {
      cancelled = true;
    };
  }, [task]);

  async function compactContext() {
    if (!task) return;
    setCompacting(true);
    try {
      await apiPost(`/api/tasks/${task.id}/compact`, {});
      toast({ tone: "success", title: "Compactação solicitada ao engine" });
      const refreshed = await apiGet<{
        session_id?: string | null;
        tokens_input: number;
        window: number;
        percent: number;
        messages: number;
      }>(`/api/tasks/${task.id}/context`);
      setContext(refreshed);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível compactar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setCompacting(false);
    }
  }

  useEffect(() => {
    if (!task) {
      setDiagnostics(null);
      setDiagnosticsError(null);
      return;
    }
    let cancelled = false;
    apiGet<TaskDiagnostics>(`/api/tasks/${task.id}/diagnostics`)
      .then((value) => {
        if (!cancelled) {
          setDiagnostics(value);
          setDiagnosticsError(null);
        }
      })
      .catch((cause) => {
        if (!cancelled) {
          setDiagnosticsError(cause instanceof Error ? cause.message : String(cause));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [task]);

  async function copyError() {
    if (!diagnostics?.error) return;
    const text = [
      `Tarefa: ${task?.title ?? ""}`,
      `Código: ${diagnostics.error.code}`,
      `Erro: ${diagnostics.error.message}`,
      `Sugestão: ${diagnostics.error.hint}`,
      diagnostics.error.run_id ? `Run: ${diagnostics.error.run_id}` : "",
      diagnostics.error.job_id ? `Job: ${diagnostics.error.job_id}` : "",
    ]
      .filter(Boolean)
      .join("\n");
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard is optional.
    }
  }

  const running = task ? isTaskRunning(task) : false;
  const queued = task?.workflow_state === "QUEUED";
  const held = task?.workflow_state === "HELD";
  const runTotals = (diagnostics?.runs ?? []).reduce(
    (totals, run) => ({
      input: totals.input + run.tokens_input,
      output: totals.output + run.tokens_output,
      cacheRead: totals.cacheRead + run.tokens_cache_read,
      cacheWrite: totals.cacheWrite + run.tokens_cache_write,
      cost: totals.cost + run.estimated_cost_usd,
    }),
    { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0 },
  );

  return (
    <Modal
      open={!!task}
      onClose={onClose}
      title={task?.title ?? "Tarefa"}
      description={
        task
          ? `${task.execution_mode} · ${task.effort_level} · ${task.assigned_agent ?? "router"}`
          : undefined
      }
      size="lg"
      initialFocus="dialog"
    >
      {task ? (
        <div className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <WorkflowStatusBadge task={task} showReason />
            <Badge
              variant="default"
              title={`Prioridade: ${task.priority} de 4 (1 é a mais alta).`}
            >
              prioridade {task.priority}
            </Badge>
            {task.assigned_agent ? (
              <EntityLink
                to={links.agent(task.assigned_agent, "workspace", task.workspace_id)}
                chip
                title={`Agente responsável: @${task.assigned_agent}`}
              >
                @{task.assigned_agent}
              </EntityLink>
            ) : (
              <Badge variant="default" title="O roteador escolhe o agente em tempo de execução.">
                agente automático
              </Badge>
            )}
            {task.preferred_model ? (
              <Badge
                variant="default"
                title="Modelo fixado nesta tarefa. O roteador não troca este modelo."
              >
                {task.preferred_model}
              </Badge>
            ) : (
              <Badge variant="default" title="O roteador escolhe o modelo conforme a política.">
                modelo automático
              </Badge>
            )}
          </div>

          {interactions.map((interaction) => (
            <InteractionCard
              key={interaction.id}
              interaction={interaction}
              onResolved={() => void loadInteractions()}
            />
          ))}
          {permissions.map((request) => (
            <PermissionRequestCard
              key={request.request_id}
              request={request}
              onDecided={() => void loadInteractions()}
            />
          ))}

          {diagnostics && diagnostics.runs.length > 0 ? (
            <div className="rounded-lg border border-zinc-800/80 bg-zinc-950/40 px-3 py-2.5">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="text-[10px] font-medium uppercase tracking-wide text-zinc-500">
                  Tokens desta tarefa
                </span>
                <span className="font-mono text-[10px] text-zinc-500">
                  {diagnostics.runs.length} execução(ões) · {formatUsd(runTotals.cost)}
                </span>
              </div>
              <div className="mt-2 grid grid-cols-2 gap-2 sm:grid-cols-4">
                <TokenStat
                  label="Entrada"
                  value={formatNumber(runTotals.input)}
                  hint="Tokens enviados ao modelo (prompts, contexto e ferramentas)."
                />
                <TokenStat
                  label="Saída"
                  value={formatNumber(runTotals.output)}
                  hint="Tokens gerados pelo modelo nas respostas."
                />
                <TokenStat
                  label="Total"
                  value={formatNumber(runTotals.input + runTotals.output)}
                  hint="Entrada + saída somadas em todas as tentativas."
                />
                <TokenStat
                  label="Cache"
                  value={`${formatNumber(runTotals.cacheRead)} / ${formatNumber(
                    runTotals.cacheWrite,
                  )}`}
                  hint="Cache lido / cache escrito, quando o provider informar."
                />
              </div>
            </div>
          ) : null}

          {task.blocked_reason ? (
            <p className="rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-2 text-[11px] font-medium text-red-200">
              Motivo do bloqueio: {task.blocked_reason}
            </p>
          ) : null}

          {running ? (
            <p className="rounded-lg border border-blue-500/20 bg-blue-500/10 px-3 py-2 text-[10px] text-blue-200">
              Esta tarefa está em execução. Cancele no controlador da fila antes de
              editar agente, modelo ou prompt.
            </p>
          ) : null}

          {sinks.length > 0 ? (
            <div className="rounded-lg border border-zinc-800/80 bg-zinc-950/40 px-3 py-2">
              <p className="text-[10px] font-medium uppercase tracking-wide text-zinc-500">
                Entregas automáticas
              </p>
              <div className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1">
                {sinks.map((sink) => (
                  <label key={sink.id} className="flex items-center gap-1.5 text-[10px] text-zinc-400">
                    <input
                      type="checkbox"
                      checked={selectedSinks.has(sink.id)}
                      onChange={(event) =>
                        setSelectedSinks((current) => {
                          const next = new Set(current);
                          if (event.target.checked) next.add(sink.id);
                          else next.delete(sink.id);
                          return next;
                        })
                      }
                      className="accent-blue-500"
                    />
                    {sink.name}
                  </label>
                ))}
                <Button
                  variant="outline"
                  size="sm"
                  className="ml-auto h-7"
                  disabled={savingSinks}
                  onClick={() => void saveSinks()}
                >
                  {savingSinks ? <Loader2 size={12} className="animate-spin" /> : null}
                  Salvar entregas
                </Button>
              </div>
            </div>
          ) : null}

          {context?.session_id ? (
            <div className="rounded-lg border border-zinc-800/80 bg-zinc-950/40 px-3 py-2">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-[10px] font-medium uppercase tracking-wide text-zinc-500">
                  contexto
                </span>
                <div className="h-1.5 min-w-32 flex-1 overflow-hidden rounded-full bg-zinc-800">
                  <div
                    className={cn(
                      "h-full rounded-full",
                      context.percent > 80
                        ? "bg-red-400"
                        : context.percent > 60
                          ? "bg-amber-400"
                          : "bg-blue-400",
                    )}
                    style={{ width: `${Math.min(100, context.percent)}%` }}
                  />
                </div>
                <span className="font-mono text-[10px] text-zinc-500">
                  {formatNumber(context.tokens_input)} / {formatNumber(context.window)} ·{" "}
                  {context.percent.toFixed(0)}% · {context.messages} msgs
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7"
                  disabled={compacting}
                  onClick={() => void compactContext()}
                >
                  {compacting ? <Loader2 size={12} className="animate-spin" /> : null}
                  Compactar contexto agora
                </Button>
              </div>
            </div>
          ) : null}

          {diagnostics?.error ? (
            <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-3 text-red-200">
              <div className="flex flex-wrap items-center gap-2">
                <AlertTriangle size={14} />
                <span className="text-[11px] font-semibold">
                  O que falhou: {diagnostics.error.code}
                </span>
                <Badge variant="red">{diagnostics.error.source}</Badge>
                <button
                  type="button"
                  onClick={() => void copyError()}
                  className="ml-auto text-[10px] text-red-300/80 underline-offset-2 hover:underline"
                >
                  {copied ? "copiado" : "copiar erro"}
                </button>
              </div>
              <p className="mt-2 whitespace-pre-wrap font-mono text-[10px] leading-relaxed">
                {diagnostics.error.message}
              </p>
              <p className="mt-2 text-[10px] leading-relaxed text-red-100/80">
                <strong>Como resolver:</strong> {diagnostics.error.hint}
              </p>
            </div>
          ) : diagnosticsError ? (
            <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
              Não foi possível carregar o diagnóstico: {diagnosticsError}
            </p>
          ) : null}

          <div>
            <p className={labelClass}>Descrição</p>
            <p className="whitespace-pre-wrap rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 text-[11px] leading-relaxed text-zinc-400">
              {task.description}
            </p>
          </div>
          {task.plan_markdown ? (
            <div>
              <p className={labelClass}>Plano</p>
              <pre className="max-h-[320px] overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 font-mono text-[10px] leading-relaxed text-zinc-300">
                {task.plan_markdown}
              </pre>
            </div>
          ) : null}

          {diagnostics && diagnostics.runs.length > 0 ? (
            <div>
              <p className={labelClass}>Últimas execuções</p>
              <div className="space-y-1.5">
                {diagnostics.runs.slice(0, 3).map((run) => (
                  <Link
                    key={run.id}
                    to={links.run(run.id)}
                    className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[10px] transition-colors hover:border-blue-500/30 hover:bg-zinc-900/50"
                  >
                    <Badge variant={runVariant(run.status)}>{run.status}</Badge>
                    <span className="font-mono text-zinc-400">{run.phase ?? "run"}</span>
                    <span className="text-zinc-500">
                      {run.agent_name} · {formatUsd(run.estimated_cost_usd)} ·{" "}
                      {Math.round(run.duration_ms / 1000)}s · ↑{" "}
                      {formatNumber(run.tokens_input)} ↓{" "}
                      {formatNumber(run.tokens_output)} ={" "}
                      {formatNumber(run.tokens_input + run.tokens_output)} tokens
                    </span>
                    <span className="ml-auto text-zinc-500">
                      {relativeTime(run.started_at)}
                    </span>
                    {run.error_message ? (
                      <p className="w-full truncate font-mono text-[9px] text-red-300/80">
                        {run.error_message}
                      </p>
                    ) : null}
                  </Link>
                ))}
              </div>
            </div>
          ) : null}

          <div className="sticky bottom-0 z-10 -mx-6 -mb-6 mt-4 flex flex-wrap items-center justify-center gap-2 border-t border-zinc-800/70 bg-zinc-950/95 px-6 py-3 backdrop-blur">
            {task.output_dir_path || diagnostics?.runs.length ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  onClose();
                  navigate(`/artifacts?task=${encodeURIComponent(task.id)}`);
                }}
                title="Abrir os arquivos gerados por esta tarefa"
              >
                <FolderOpen size={13} />
                Ver arquivos
              </Button>
            ) : null}
            <Button
              variant="outline"
              size="sm"
              onClick={() => onFork(task)}
              title="Revisar e criar uma cópia independente desta tarefa"
            >
              <GitFork size={13} />
              Fork
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => onSwarm(task)}
              title="Rodar a mesma tarefa em 2 modelos e comparar os entregáveis"
            >
              <Columns3 size={13} />
              A/B
            </Button>
            <Button variant="outline" size="sm" onClick={() => onEdit(task)}>
              <Pencil size={13} />
              Editar
            </Button>
            <Button variant="outline" size="sm" onClick={() => onLogs(task)}>
              <FileText size={13} />
              Execução
            </Button>
            {task.status !== "CANCELLED" && task.status !== "DONE" ? (
              <Button
                variant="destructive"
                size="sm"
                onClick={() => onStatus(task, "CANCELLED")}
                title="Cancelar imediatamente e interromper a execução"
              >
                <Square size={12} />
                Cancelar
              </Button>
            ) : null}
            {task.status !== "BLOCKED" && task.status !== "DONE" ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onStatus(task, "BLOCKED")}
                title="Bloquear a tarefa e interromper a execução"
              >
                <Ban size={12} />
                Bloquear
              </Button>
            ) : null}
            {!held && (task.status === "IN_PROGRESS" || task.status === "TODO") ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onWait(task)}
                title="Colocar em espera e impedir o início automático"
              >
                <Pause size={12} />
                Em espera
              </Button>
            ) : null}
            {held ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onStatus(task, "TODO")}
                title="Retomar e entrar na fila automaticamente"
              >
                <Play size={12} />
                Retomar
              </Button>
            ) : null}
            {task.status === "BLOCKED" || task.status === "CANCELLED" || task.status === "DONE" ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onStatus(task, "TODO")}
                title="Reabrir a tarefa como To do"
              >
                <RotateCw size={12} />
                Reabrir
              </Button>
            ) : null}
            {task.status !== "DONE" ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onStatus(task, "DONE")}
                title="Marcar como concluída sem executar"
              >
                <CheckCircle2 size={12} />
                Concluir
              </Button>
            ) : null}
            {isTaskAwaitingApproval(task) ? (
              <Button size="sm" onClick={() => onApprove(task)}>
                <CheckCircle2 size={13} />
                Aprovar plano e iniciar Build
              </Button>
            ) : !running && !queued && !held ? (
              <Button
                size="sm"
                onClick={() => onDispatch(task)}
                disabled={!daemon?.online}
                title={
                  daemon?.online
                    ? "Executar agora"
                    : "OpenCode engine offline: inicie o motor para executar"
                }
              >
                <Play size={13} />
                {task.status === "BLOCKED" ? "Nova tentativa" : "Executar"}
              </Button>
            ) : null}
          </div>
        </div>
      ) : null}
    </Modal>
  );
}

function TokenStat({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint: string;
}) {
  return (
    <div
      className="rounded-md border border-zinc-800/70 bg-zinc-900/30 px-2 py-1.5"
      title={hint}
    >
      <p className="text-[9px] uppercase tracking-wide text-zinc-500">{label}</p>
      <p className="mt-0.5 font-mono text-[12px] text-zinc-200">{value}</p>
    </div>
  );
}

function runVariant(status: string): "green" | "red" | "blue" | "yellow" | "default" {
  if (status === "SUCCESS") return "green";
  if (status === "FAILED") return "red";
  if (status === "RUNNING") return "blue";
  if (status === "CANCELLED") return "yellow";
  return "default";
}


export function LogDrawer({
  task,
  initialRunId,
  onClose,
  lastRunEvent,
}: {
  task: Task | null;
  initialRunId?: string | null;
  onClose: () => void;
  lastRunEvent: { run_id: string; status: string } | null;
}) {
  const [runs, setRuns] = useState<TaskRun[]>([]);
  const [selectedRun, setSelectedRun] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  useEffect(() => {
    if (!task) return;
    let cancelled = false;
    setLoading(true);
    apiGet<TaskRun[]>(
      `/api/runs?task_id=${encodeURIComponent(task.id)}&workspace_id=${encodeURIComponent(task.workspace_id)}`,
    )
      .then((items) => {
        if (cancelled) return;
        setRuns(items);
        setSelectedRun((current) => {
          if (initialRunId && items.some((item) => item.id === initialRunId)) {
            return initialRunId;
          }
          return current ?? items[0]?.id ?? null;
        });
      })
      .catch((cause) =>
        setError(cause instanceof Error ? cause.message : String(cause)),
      )
      .finally(() => setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [task, initialRunId, lastRunEvent?.run_id, lastRunEvent?.status]);

  if (!task) return null;

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center p-3 sm:p-5">
      <button
        type="button"
        aria-label="Fechar logs"
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={onClose}
      />
      <aside className="relative flex h-[min(88dvh,64rem)] w-[min(94vw,72rem)] flex-col overflow-hidden rounded-2xl border border-zinc-800 bg-zinc-950/95 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] backdrop-blur-xl">
        <div className="flex items-center justify-between border-b border-zinc-800/70 px-4 py-3">
          <div className="min-w-0">
            <p className="truncate text-[12px] font-medium text-zinc-200">
              {task.title}
            </p>
            <p className="font-mono text-[9px] text-zinc-500">
              {STATUS_LABEL[task.status] ?? task.status} · {task.phase_status ?? "sem fase"}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {selectedRun ? (
              <Link
                to={links.run(selectedRun)}
                className="rounded-lg border border-zinc-800 px-2.5 py-1 text-[10px] text-zinc-400 hover:border-zinc-700 hover:text-zinc-200"
              >
                abrir página
              </Link>
            ) : null}
            <Button variant="ghost" size="sm" onClick={onClose}>
              <XCircle size={14} />
              Fechar
            </Button>
          </div>
        </div>
        <div className="flex min-h-0 flex-1">
          <div className="w-56 shrink-0 overflow-y-auto border-r border-zinc-800/70 p-2">
            {loading && runs.length === 0 ? (
              <p className="p-2 text-[10px] text-zinc-500">Carregando…</p>
            ) : error ? (
              <p className="p-2 text-[10px] text-red-300">{error}</p>
            ) : runs.length === 0 ? (
              <p className="p-2 text-[10px] text-zinc-500">Nenhum run.</p>
            ) : (
              runs.map((run) => (
                <button
                  key={run.id}
                  type="button"
                  onClick={() => setSelectedRun(run.id)}
                  className={cn(
                    "mb-1 w-full rounded-lg px-2 py-1.5 text-left",
                    selectedRun === run.id
                      ? "bg-zinc-800 text-zinc-100"
                      : "text-zinc-500 hover:bg-zinc-900",
                  )}
                >
                  <div className="flex items-center justify-between gap-1">
                    <p className="truncate text-[10px]">
                      tentativa {run.attempt_number ?? 1} · {run.phase ?? "run"}
                    </p>
                    <Badge variant={runVariant(run.status)}>{run.status}</Badge>
                  </div>
                  <p className="font-mono text-[8px] text-zinc-500">
                    {formatUsd(run.estimated_cost_usd)} · {relativeTime(run.started_at)}
                  </p>
                  {run.error_message ? (
                    <p className="truncate text-[8px] text-red-400/80">
                      {run.error_message}
                    </p>
                  ) : null}
                </button>
              ))
            )}
          </div>
          <div className="flex min-w-0 flex-1 flex-col overflow-hidden bg-zinc-950/60">
            {selectedRun ? (
              <RunLogView runId={selectedRun} compact />
            ) : (
              <EmptyState
                icon={FileText}
                title="Nenhum run selecionado"
                description="Execute a tarefa ou escolha um run na lista à esquerda."
                className="border-0 bg-transparent"
              />
            )}
          </div>
        </div>
      </aside>
    </div>
  );
}

const labelClass =
  "mb-1.5 block text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500";


