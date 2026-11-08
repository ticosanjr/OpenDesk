import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  AlertTriangle,
  Ban,
  CheckCircle2,
  Columns3,
  FileText,
  GitBranch,
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
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/modal";
import { apiGet, apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import { STATUS_LABEL } from "@/lib/task-ui";
import type { Task, TaskDiagnostics, TaskRun } from "@/lib/types";
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
  const daemon = useAppStore((state) => state.daemon);

  useEffect(() => {
    if (!task) {
      setSinks([]);
      setSelectedSinks(new Set());
      return;
    }
    let cancelled = false;
    void Promise.all([
      apiGet<{ sinks: OutboundSink[] }>(
        `/api/io/outbound?workspace_id=${encodeURIComponent(task.workspace_id)}`,
      ),
      apiGet<{ bindings: Array<{ sink_id: string }> }>(
        `/api/tasks/${task.id}/sinks`,
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
  }, [task?.id]);

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

  const running =
    task?.status === "IN_PROGRESS" ||
    task?.phase_status === "queued" ||
    task?.phase_status === "running" ||
    task?.phase_status === "follow_up_queued";
  const failed =
    task?.phase_status?.includes("failed") ||
    diagnostics?.error !== null && diagnostics?.error !== undefined;

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
      className="max-w-3xl"
    >
      {task ? (
        <div className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <Badge
              variant={
                task.status === "BLOCKED"
                  ? "red"
                  : task.status === "DONE"
                    ? "green"
                    : task.status === "IN_PROGRESS"
                      ? "blue"
                      : task.status === "CANCELLED"
                        ? "default"
                        : "yellow"
              }
            >
              {STATUS_LABEL[task.status] ?? task.status}
            </Badge>
            <Badge variant="default">prioridade {task.priority}</Badge>
            {task.last_run_status ? (
              <Badge variant={runVariant(task.last_run_status)}>
                último run: {task.last_run_status}
              </Badge>
            ) : (
              <Badge variant="default">sem execução</Badge>
            )}
            {task.phase_status ? (
              <Badge variant={failed ? "red" : running ? "blue" : "yellow"}>
                {task.phase_status}
              </Badge>
            ) : null}
            {task.assigned_agent ? (
              <EntityLink
                to={links.agent(task.assigned_agent, "workspace", task.workspace_id)}
                chip
              >
                @{task.assigned_agent}
              </EntityLink>
            ) : null}
            {task.preferred_model ? (
              <Badge variant="default">{task.preferred_model}</Badge>
            ) : null}
            {task.output_dir_path ? (
              <Badge variant="blue" title={task.output_dir_path}>
                output
              </Badge>
            ) : null}
          </div>

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
                      {Math.round(run.duration_ms / 1000)}s
                    </span>
                    <span className="ml-auto text-zinc-600">
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

          <div className="flex flex-wrap justify-end gap-2 border-t border-zinc-800/70 pt-4">
            <Button variant="ghost" size="sm" onClick={onClose}>
              Fechar
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => onFork(task)}
              title="Criar uma cópia independente desta tarefa e do estado atual"
            >
              <GitBranch size={13} />
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
            {task.status === "IN_PROGRESS" || task.status === "TODO" ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onWait(task)}
                title="Colocar em espera sem executar"
              >
                <Pause size={12} />
                Em espera
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
            {task.phase_status === "awaiting_approval" ? (
              <Button size="sm" onClick={() => onApprove(task)}>
                <CheckCircle2 size={13} />
                Aprovar plano e iniciar Build
              </Button>
            ) : task.status !== "IN_PROGRESS" ? (
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
    <div className="fixed inset-0 z-40 flex justify-end">
      <button
        type="button"
        aria-label="Fechar logs"
        className="absolute inset-0 bg-black/60"
        onClick={onClose}
      />
      <aside className="relative flex h-dvh w-full max-w-5xl flex-col border-l border-zinc-800 bg-zinc-950 shadow-2xl">
        <div className="flex items-center justify-between border-b border-zinc-800/70 px-4 py-3">
          <div className="min-w-0">
            <p className="truncate text-[12px] font-medium text-zinc-200">
              {task.title}
            </p>
            <p className="font-mono text-[9px] text-zinc-600">
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
              <p className="p-2 text-[10px] text-zinc-600">Carregando…</p>
            ) : error ? (
              <p className="p-2 text-[10px] text-red-300">{error}</p>
            ) : runs.length === 0 ? (
              <p className="p-2 text-[10px] text-zinc-600">Nenhum run.</p>
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
                  <p className="font-mono text-[8px] text-zinc-600">
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

  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60";
const labelClass =
  "mb-1.5 block text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500";


