import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  Loader2,
  Play,
  RotateCw,
  Square,
  TerminalSquare,
} from "lucide-react";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { apiGet, apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import { isTaskAwaitingApproval, taskState } from "@/lib/task-state";
import type { DispatchDecision, Task } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";

const MODES = [
  ["AUTO_DSH", "AUTO_DSH — roteador decide"],
  ["PLAN_ONLY", "PLAN_ONLY — só plano"],
  ["PLAN_THEN_BUILD", "PLAN_THEN_BUILD — aprovar plano"],
  ["BUILD_DIRECT", "BUILD_DIRECT — executar direto"],
] as const;

const EFFORTS = ["low", "medium", "high"] as const;

const STATE_LABEL: Record<string, string> = {
  idle: "pronta",
  queued: "na fila",
  running: "em execução",
  needs_approval: "aguardando aprovação",
  waiting_input: "aguardando resposta",
  blocked: "bloqueada",
  done: "concluída",
  cancelled: "cancelada",
  held: "em espera",
};

export function ExecutionModal({
  open,
  task,
  onClose,
  onChanged,
}: {
  open: boolean;
  task: Task | null;
  onClose: () => void;
  onChanged?: () => void | Promise<void>;
}) {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const daemon = useAppStore((state) => state.daemon);
  const workspaceId = task?.workspace_id ?? activeWorkspaceId;
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const [current, setCurrent] = useState<Task | null>(null);
  const [mode, setMode] = useState("AUTO_DSH");
  const [agent, setAgent] = useState("");
  const [model, setModel] = useState<string | null>(null);
  const [effort, setEffort] = useState("medium");
  const [extra, setExtra] = useState("");
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const taskId = task?.id ?? null;

  useEffect(() => {
    if (!open || !task) {
      setCurrent(null);
      return;
    }
    setCurrent(task);
    setMode(task.execution_mode);
    setAgent(task.assigned_agent ?? "");
    setModel(task.preferred_model ?? null);
    setEffort(task.effort_level);
    setExtra("");
    setError(null);
  }, [open, task]);

  const refresh = useCallback(async () => {
    if (!taskId) return;
    try {
      const value = await apiGet<Task>(`/api/tasks/${encodeURIComponent(taskId)}`);
      setCurrent(value);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [taskId]);

  useEffect(() => {
    if (!open || !taskId) return;
    void refresh();
    const timer = window.setInterval(() => void refresh(), 4_000);
    return () => window.clearInterval(timer);
  }, [open, taskId, lastTaskEvent, lastRunEvent, refresh]);

  if (!task) return null;

  const status = current ? taskState(current) : "idle";
  const running = status === "running" || status === "queued";
  const awaitingApproval = current ? isTaskAwaitingApproval(current) : false;

  async function start() {
    if (!task) return;
    setStarting(true);
    setError(null);
    try {
      const body = {
        manual_agent: agent.trim() || null,
        manual_model: model,
        manual_effort: effort,
        execution_mode: mode,
        prompt_append: extra.trim() || null,
      };
      const response = await apiPost<{ decision: DispatchDecision }>(
        `/api/tasks/${task.id}/${awaitingApproval ? "approve-plan" : "dispatch"}`,
        body,
      );
      toast({
        tone: "success",
        title: awaitingApproval ? "Plano aprovado e execução iniciada" : "Execução iniciada",
        description: `${response.decision.phase} · @${response.decision.selected_agent} · ${response.decision.selected_model}`,
        href: links.task(task.id),
      });
      await refresh();
      await onChanged?.();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setStarting(false);
    }
  }

  async function stop() {
    if (!task) return;
    setStopping(true);
    setError(null);
    try {
      await apiPost(`/api/tasks/${task.id}/transition`, { status: "CANCELLED" });
      toast({
        tone: "warning",
        title: "Execução interrompida",
        description: "A tarefa foi cancelada e o job ativo foi encerrado.",
      });
      await refresh();
      await onChanged?.();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setStopping(false);
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Executar tarefa"
      description={task.title}
      size="xl"
    >
      <div className="space-y-3">
        <div className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
          <Badge
            variant={
              status === "running"
                ? "blue"
                : status === "queued"
                  ? "yellow"
                  : status === "blocked"
                    ? "red"
                    : status === "done"
                      ? "green"
                      : "default"
            }
          >
            {STATE_LABEL[status] ?? status}
          </Badge>
          {current?.phase ? (
            <span className="font-mono text-[11.5px] text-zinc-500">
              fase {current.phase}
            </span>
          ) : null}
          {current?.last_run_status ? (
            <span className="font-mono text-[11.5px] text-zinc-500">
              último run: {current.last_run_status}
            </span>
          ) : null}
          {current?.output_dir_path ? (
            <span className="min-w-0 truncate font-mono text-[11.5px] text-zinc-600">
              {current.output_dir_path}
            </span>
          ) : null}
          <span className="ml-auto flex items-center gap-1.5">
            <Button variant="ghost" size="xs" onClick={() => void refresh()}>
              <RotateCw size={11} />
              atualizar
            </Button>
            {current?.last_run_id ? (
              <Link
                to={links.run(current.last_run_id)}
                className="text-[11.5px] text-zinc-400 hover:text-zinc-200"
              >
                ver execução
              </Link>
            ) : null}
          </span>
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <label>
            <span className={labelClass}>Modo</span>
            <select
              value={mode}
              onChange={(event) => setMode(event.target.value)}
              className={inputClass}
              disabled={running}
            >
              {MODES.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className={labelClass}>Effort</span>
            <select
              value={effort}
              onChange={(event) => setEffort(event.target.value)}
              className={inputClass}
              disabled={running}
            >
              {EFFORTS.map((value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ))}
            </select>
          </label>
          <div>
            <span className={labelClass}>Agente</span>
            <AgentCombobox
              value={agent}
              onChange={(value) => setAgent(value)}
              workspaceId={workspaceId}
              placeholder="automático (router)"
            />
          </div>
          <div>
            <span className={labelClass}>Modelo</span>
            <ModelCombobox
              value={model}
              onChange={setModel}
              placeholder="roteador automático"
            />
          </div>
        </div>

        <div>
          <span className={labelClass}>
            Comando, skill ou instrução desta execução
          </span>
          <SlashTextarea
            value={extra}
            onChange={setExtra}
            workspaceId={workspaceId}
            rows={4}
            placeholder="Opcional. Digite / para inserir um comando ou skill; o texto entra só neste run."
          />
          <p className="mt-1 text-[11.5px] leading-snug text-zinc-500">
            Comandos e skills são anexados ao prompt desta execução. A descrição
            da tarefa e as configurações salvas não mudam.
          </p>
        </div>

        {running ? (
          <p className="flex items-start gap-2 rounded-lg border border-blue-500/20 bg-blue-500/[0.06] px-3 py-2 text-[12.5px] text-blue-200">
            <TerminalSquare size={13} className="mt-0.5 shrink-0" />
            A execução está ativa. Você pode acompanhar em “ver execução” ou
            interromper agora; a tarefa fica cancelada e pode ser reaberta depois.
          </p>
        ) : null}

        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
            {error}
          </p>
        ) : null}

        <div className="flex flex-wrap items-center justify-end gap-2 border-t border-zinc-800/70 pt-3">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Fechar
          </Button>
          {running ? (
            <Button
              variant="destructive"
              size="sm"
              onClick={() => void stop()}
              disabled={stopping}
            >
              {stopping ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Square size={12} />
              )}
              Parar execução
            </Button>
          ) : (
            <Button
              size="sm"
              onClick={() => void start()}
              disabled={starting || !daemon?.online}
              title={
                daemon?.online
                  ? "Iniciar agora"
                  : "OpenCode offline: inicie o motor para executar"
              }
            >
              {starting ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Play size={13} />
              )}
              {awaitingApproval ? "Aprovar plano e executar" : "Executar agora"}
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
}
