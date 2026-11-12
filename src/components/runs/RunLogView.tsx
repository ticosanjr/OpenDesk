import { useCallback, useEffect, useState } from "react";
import {
  BookmarkPlus,
  Braces,
  CheckCircle2,
  Copy,
  FileText,
  FolderOpen,
  FolderTree,
  GitFork,
  Loader2,
  PlayCircle,
  RefreshCw,
  Send,
  Terminal,
  XCircle,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { apiDelete, apiGet, apiPost, apiText } from "@/lib/api";
import type {
  RunTimeline,
  Task,
  TaskInteraction,
  TaskMessage,
} from "@/lib/types";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { WorkingIndicator } from "@/components/ui/working-indicator";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EntityLink } from "@/components/ui/entity-link";
import { CountPill } from "@/components/ui/count-pill";
import { IconLabel } from "@/components/ui/icon-label";
import { EmptyState } from "@/components/ui/empty-state";
import { ForkTaskModal } from "@/components/kanban/task-form-modal";
import {
  InteractionCard,
  PermissionRequestCard,
} from "@/components/interactions/InteractionCard";
import { links } from "@/lib/links";
import { runStatusLabel } from "@/lib/task-ui";
import {
  formatClock,
  formatDuration,
  formatNumber,
  formatUsd,
  relativeTime,
} from "@/lib/format";
import { toast } from "@/store/toast-store";
import { useAppStore } from "@/store/app-store";
import { cn } from "@/lib/utils";
import {
  Band,
  BrainIcon,
  StreamView,
  ToolRow,
} from "@/components/runs/run-log-parts";

type Tone = "default" | "blue" | "green" | "yellow" | "red" | "purple";

function statusVariant(status: string): Tone {
  if (status === "SUCCESS") return "green";
  if (status === "FAILED") return "red";
  if (status === "RUNNING") return "blue";
  if (status === "CANCELLED") return "yellow";
  return "default";
}

export function RunLogView({
  runId,
  compact = false,
}: {
  runId: string;
  compact?: boolean;
}) {
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastRunChunk = useAppStore((state) => state.lastRunChunk);
  const [timeline, setTimeline] = useState<RunTimeline | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [raw, setRaw] = useState<string | null>(null);
  const [rawLoading, setRawLoading] = useState(false);
  const [showRaw, setShowRaw] = useState(false);
  const [view, setView] = useState<"stream" | "bands">("stream");
  const [messages, setMessages] = useState<TaskMessage[]>([]);
  const [composer, setComposer] = useState("");
  const [composerAgent, setComposerAgent] = useState("");
  const [composerModel, setComposerModel] = useState<string | null>(null);
  const [continueExecution, setContinueExecution] = useState(true);
  const [sending, setSending] = useState(false);
  const [follow, setFollow] = useState(false);
  const [forkSource, setForkSource] = useState<Task | null>(null);
  const [forkLoading, setForkLoading] = useState(false);
  const [openingFolder, setOpeningFolder] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
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
  const lastPermissionEvent = useAppStore((state) => state.lastPermissionEvent);

  const loadMessages = useCallback(async (taskId: string) => {
    try {
      const list = await apiGet<TaskMessage[]>(
        `/api/tasks/${encodeURIComponent(taskId)}/messages`,
      );
      setMessages(Array.isArray(list) ? list : []);
    } catch {
      setMessages([]);
    }
  }, []);

  const load = useCallback(async () => {
    try {
      const value = await apiGet<RunTimeline>(
        `/api/runs/${encodeURIComponent(runId)}/thread`,
      );
      setTimeline(value);
      if (value.run?.task_id) void loadMessages(value.run.task_id);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [loadMessages, runId]);

  const loadInteractions = useCallback(async () => {
    const taskId = timeline?.run?.task_id;
    const workspaceId = timeline?.run?.workspace_id;
    if (!taskId || !workspaceId) {
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
          `/api/permissions?workspace_id=${encodeURIComponent(workspaceId)}`,
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
          (request) => request.task_id === taskId || request.run_id === runId,
        ),
      );
    } catch {
      setInteractions([]);
      setPermissions([]);
    }
  }, [runId, timeline?.run?.task_id, timeline?.run?.workspace_id]);

  useEffect(() => {
    void loadInteractions();
  }, [loadInteractions, lastPermissionEvent?.request_id]);

  useEffect(() => {
    setLoading(true);
    setTimeline(null);
    setRaw(null);
    setShowRaw(false);
    void load();
  }, [load]);

  useEffect(() => {
    if (lastRunEvent?.run_id === runId) void load();
  }, [lastRunEvent?.run_id, lastRunEvent?.status, load, runId]);

  useEffect(() => {
    if (lastRunChunk?.run_id !== runId) return;
    const handle = window.setTimeout(() => void load(), 250);
    return () => window.clearTimeout(handle);
  }, [lastRunChunk?.seq, lastRunChunk?.run_id, load, runId]);

  const running =
    timeline?.status === "RUNNING" ||
    timeline?.run?.status === "RUNNING" ||
    loading;

  useEffect(() => {
    if (!running && !follow) return;
    const handle = window.setInterval(() => void load(), 1000);
    return () => window.clearInterval(handle);
  }, [follow, load, running]);

  useEffect(() => {
    if (!follow) return;
    const handle = window.setTimeout(() => setFollow(false), 60_000);
    return () => window.clearTimeout(handle);
  }, [follow]);

  async function sendComment() {
    if (!timeline?.run?.task_id || !composer.trim()) return;
    setSending(true);
    try {
      await apiPost(`/api/tasks/${timeline.run.task_id}/messages`, {
        content: composer.trim(),
        continue_execution: continueExecution,
        agent: composerAgent.trim() || null,
        model: composerModel,
      });
      setComposer("");
      toast({
        tone: "success",
        title: continueExecution
          ? "Mensagem enviada; continuação enfileirada"
          : "Comentário adicionado",
      });
      if (continueExecution) setFollow(true);
      await Promise.all([load(), loadMessages(timeline.run.task_id)]);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível enviar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSending(false);
    }
  }

  async function loadRaw() {
    setRawLoading(true);
    try {
      const value = await apiText(`/api/runs/${runId}/log`);
      setRaw(value);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setRawLoading(false);
    }
  }

  async function pinMemory() {
    const taskId = timeline?.run?.task_id;
    const text = timeline?.result.text?.trim() || summary || "";
    if (!taskId || !text) return;
    try {
      await apiPost(`/api/tasks/${taskId}/memory`, {
        content: text,
        run_id: runId,
      });
      toast({
        tone: "success",
        title: "Salvo em .opencode/MEMORY.md",
        description:
          "O resultado entra no contexto dos próximos runs. Você pode desfazer agora.",
        href: links.file(".opencode/MEMORY.md"),
        action: {
          label: "Desfazer",
          onClick: () => {
            void apiDelete(
              `/api/tasks/${taskId}/memory?run_id=${encodeURIComponent(runId)}`,
            )
              .then(() =>
                toast({ tone: "info", title: "Entrada removida da memória" }),
              )
              .catch((cause) =>
                toast({
                  tone: "error",
                  title: "Não foi possível desfazer",
                  description:
                    cause instanceof Error ? cause.message : String(cause),
                }),
              );
          },
        },
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar na memória",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function forkHere() {
    const taskId = timeline?.run?.task_id;
    if (!taskId) return;
    setForkLoading(true);
    try {
      const task = await apiGet<Task>(`/api/tasks/${taskId}`);
      setForkSource(task);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível abrir o fork",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setForkLoading(false);
    }
  }

  async function openOutputFolder() {
    const path = timeline?.output_directory;
    if (!path) return;
    setOpeningFolder(true);
    try {
      await openPath(path);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível abrir a pasta",
        description: `${path} · ${cause instanceof Error ? cause.message : String(cause)}`,
        action: {
          label: "Copiar caminho",
          onClick: () => {
            void navigator.clipboard.writeText(path);
          },
        },
      });
    } finally {
      setOpeningFolder(false);
    }
  }

  async function refresh() {
    setRefreshing(true);
    const started = Date.now();
    try {
      await load();
    } finally {
      const remaining = 300 - (Date.now() - started);
      if (remaining > 0) {
        await new Promise((resolve) => window.setTimeout(resolve, remaining));
      }
      setRefreshing(false);
    }
  }

  function copyExecution() {
    const lines: string[] = [];
    for (const message of messages) {
      lines.push(`Você: ${message.content}`);
    }
    for (const event of timeline?.events ?? []) {
      if (event.kind === "comment" && event.text.trim()) {
        lines.push(`Assistente: ${event.text.trim()}`);
      } else if (event.kind === "tool" && event.tool) {
        lines.push(
          `[ferramenta] ${event.tool.summary} (${event.tool.status})`,
        );
      }
    }
    const result = timeline?.result.text?.trim() || summary;
    if (result) lines.push(`Resultado: ${result}`);
    void copy(lines.join("\n\n"));
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast({ tone: "success", title: "Copiado" });
    } catch {
      toast({ tone: "error", title: "Não foi possível copiar" });
    }
  }

  const run = timeline?.run;
  const status = timeline?.status ?? run?.status ?? "RUNNING";
  const failedRun = status === "FAILED" || status === "CANCELLED";
  const summary = run?.summary_response?.trim() || timeline?.result.text;
  const attempts = timeline?.attempts ?? [];
  const totalTokens = attempts.length
    ? attempts.reduce(
        (total, attempt) =>
          total + attempt.tokens_input + attempt.tokens_output,
        0,
      )
    : (run?.tokens_input ?? 0) + (run?.tokens_output ?? 0);
  const totalInput = attempts.length
    ? attempts.reduce((total, attempt) => total + attempt.tokens_input, 0)
    : run?.tokens_input ?? 0;
  const totalOutput = attempts.length
    ? attempts.reduce((total, attempt) => total + attempt.tokens_output, 0)
    : run?.tokens_output ?? 0;
  const totalCost = attempts.length
    ? attempts.reduce((total, attempt) => total + attempt.cost_usd, 0)
    : run?.estimated_cost_usd ?? 0;

  if (loading && !timeline) {
    return (
      <div className="flex items-center gap-2 p-6 text-[11px] text-zinc-500">
        <Loader2 size={14} className="animate-spin" />
        Carregando execução…
      </div>
    );
  }

  if (error && !timeline) {
    return (
      <div className="space-y-3 p-4">
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
        <Button size="sm" variant="outline" onClick={() => void load()}>
          <RefreshCw size={13} />
          Tentar novamente
        </Button>
      </div>
    );
  }

  return (
    <div className={cn("flex h-full min-h-0 flex-col", compact ? "" : "gap-3")}>
      <div className="shrink-0 border-b border-zinc-800/70 px-4 py-3">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant={statusVariant(status)}>{runStatusLabel(status)}</Badge>
          {status === "RUNNING" ? <WorkingIndicator label="executando" /> : null}
          {timeline?.status === "RUNNING" || timeline?.log_missing === false ? (
            <span className="inline-flex items-center gap-1 font-mono text-[11px] text-zinc-500">
              <span
                className={cn(
                  "h-1.5 w-1.5 rounded-full",
                  status === "RUNNING" ? "animate-pulse bg-blue-400" : "bg-emerald-400",
                )}
              />
              {status === "RUNNING" ? "ao vivo" : "encerrado"}
            </span>
          ) : null}
          <span className="font-mono text-[11.5px] text-zinc-500">
            {formatDuration(timeline?.duration_ms ?? run?.duration_ms)}
          </span>
          {attempts.length > 1 ? (
            <Badge
              variant="purple"
              title={`Esta execução tem ${attempts.length} tentativas. A linha do tempo abaixo é contínua.`}
            >
              {attempts.length} tentativas
            </Badge>
          ) : null}
          {timeline?.steps.length ? (
            <Badge variant="default">{timeline.steps.length} passos</Badge>
          ) : null}
          <span className="ml-auto flex flex-wrap items-center gap-1.5">
            {run?.task_id ? (
              <EntityLink
                to={links.task(run.task_id)}
                chip
                title="Abrir o card da tarefa"
              >
                <IconLabel icon={<FileText size={10} />}>abrir tarefa</IconLabel>
              </EntityLink>
            ) : null}
            {timeline?.artifacts.length ? (
              <EntityLink
                to={`/artifacts?run=${encodeURIComponent(runId)}`}
                chip
                title="Abrir os artefatos desta execução"
              >
                <IconLabel icon={<FileText size={10} />}>artefatos</IconLabel>
                <CountPill value={timeline.artifacts.length} tone="blue" />
              </EntityLink>
            ) : null}
            {timeline?.output_directory ? (
              <>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-8"
                  onClick={() => void openOutputFolder()}
                  disabled={openingFolder}
                  title={
                    openingFolder
                      ? "Abrindo pasta…"
                      : `Abrir a pasta de output no sistema (${timeline.output_directory})`
                  }
                >
                  <IconLabel
                    icon={
                      openingFolder ? (
                        <Loader2 size={12} className="animate-spin" />
                      ) : (
                        <FolderOpen size={12} />
                      )
                    }
                  >
                    pasta
                  </IconLabel>
                </Button>
                {timeline.output_directory_relative ? (
                  <EntityLink
                    to={links.dir(timeline.output_directory_relative)}
                    chip
                    title="Abrir a pasta de output no explorador do OpenDesk"
                  >
                    <IconLabel icon={<FolderTree size={10} />}>
                      explorador
                    </IconLabel>
                  </EntityLink>
                ) : null}
              </>
            ) : null}
            {timeline?.run?.task_id ? (
              <>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7"
                  onClick={() => void pinMemory()}
                  title="Salvar o resultado em .opencode/MEMORY.md; você pode desfazer no aviso"
                >
                  <IconLabel icon={<BookmarkPlus size={12} />}>memória</IconLabel>
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7"
                  onClick={() => void forkHere()}
                  disabled={forkLoading}
                  title="Revisar e criar uma tarefa independente com os artefatos deste run"
                >
                  {forkLoading ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <GitFork size={12} />
                  )}
                  fork
                </Button>
              </>
            ) : null}
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={copyExecution}
              title="Copiar a transcrição completa desta execução"
            >
              <IconLabel icon={<Copy size={12} />}>copiar execução</IconLabel>
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={() => setView(view === "stream" ? "bands" : "stream")}
              title="Alternar entre linha do tempo e resumo agrupado"
            >
              {view === "stream" ? "resumo" : "linha do tempo"}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={() => {
                setShowRaw((current) => !current);
                if (!raw && !showRaw) void loadRaw();
              }}
            >
              <Braces size={12} />
              {showRaw ? "timeline" : "bruto"}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={() => void refresh()}
              title="Recarregar a execução"
              disabled={refreshing}
            >
              <RefreshCw
                size={12}
                className={refreshing ? "animate-spin" : undefined}
              />
            </Button>
          </span>
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 font-mono text-[9px] text-zinc-500">
          <span>{run?.agent_name ?? "agente"}</span>
          <span>{run?.model_id ?? "modelo"}</span>
          {run ? (
            <>
              <span>
                ↑ {formatNumber(totalInput)} · ↓{" "}
                {formatNumber(totalOutput)} · Σ{" "}
                {formatNumber(totalTokens)} tokens
              </span>
              <span>{formatUsd(totalCost)}</span>
            </>
          ) : null}
          <span>
            início {formatClock(timeline?.started_at ?? run?.started_at)}
          </span>
          <span>
            fim {formatClock(timeline?.completed_at ?? run?.completed_at)}
          </span>
          {run?.started_at ? <span>{relativeTime(run.started_at)}</span> : null}
        </div>
        <div className="mt-2 flex items-center gap-2">
          <span className="font-mono text-[10px] text-zinc-500">contexto</span>
          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-zinc-800">
            <div
              className={cn(
                "h-full rounded-full",
                totalTokens > 102_400 ? "bg-red-400" : totalTokens > 81_920 ? "bg-amber-400" : "bg-blue-400",
              )}
              style={{ width: `${Math.min(100, (totalTokens / 128_000) * 100)}%` }}
            />
          </div>
          <span className="font-mono text-[10px] text-zinc-500">
            {formatNumber(totalTokens)} / 128k
          </span>
        </div>
      </div>

      {timeline?.artifacts.length ? (
        <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-zinc-800/70 bg-zinc-950/40 px-4 py-2">
          <span className="mr-1 font-mono text-[9px] uppercase tracking-wide text-zinc-500">
            entregáveis
          </span>
          {timeline.artifacts.slice(0, 8).map((artifact) => (
            <EntityLink key={artifact.id} to={links.artifact(artifact.id)} chip>
              <FileText size={9} />
              {artifact.file_path.split("/").pop() ?? artifact.file_path}
            </EntityLink>
          ))}
          {timeline.artifacts.length > 8 ? (
            <EntityLink to={`/artifacts?run=${encodeURIComponent(runId)}`} chip>
              +{timeline.artifacts.length - 8}
            </EntityLink>
          ) : null}
        </div>
      ) : null}

      {showRaw ? (
        <div className="min-h-0 flex-1 overflow-auto p-4">
          {rawLoading ? (
            <p className="flex items-center gap-2 text-[10px] text-zinc-500">
              <Loader2 size={12} className="animate-spin" />
              Carregando log bruto…
            </p>
          ) : (
            <pre className="whitespace-pre-wrap break-words font-mono text-[9px] leading-relaxed text-zinc-400">
              {raw || "Log bruto indisponível."}
            </pre>
          )}
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="min-h-0 flex-1 space-y-2 overflow-auto p-3">
          {view === "bands" ? (
            <>
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
            </>
          ) : null}
          {view === "stream" ? (
            <StreamView
              timeline={timeline}
              messages={messages}
              workspaceRoot={timeline?.workspace_root}
              summary={summary}
              failedRun={failedRun}
              copy={copy}
              interactions={interactions}
              permissions={permissions}
              onInteractionsChanged={() => void loadInteractions()}
            />
          ) : (
            <>
          {timeline?.log_missing ? (
            <EmptyState
              icon={FileText}
              title="Log não encontrado"
              description="Os metadados da execução existem, mas o arquivo de log não está mais no disco."
              className="border-0 bg-transparent"
            />
          ) : null}

          {timeline?.steps.length ? (
            <div className="flex flex-wrap gap-1.5 px-1">
              {timeline.steps.map((step) => (
                <span
                  key={step.index}
                  className="inline-flex items-center gap-1 rounded-md border border-zinc-800/70 bg-zinc-950/40 px-1.5 py-1 font-mono text-[9px] text-zinc-500"
                  title={`${step.agent ?? "agente"} · ${step.model ?? "modelo"}`}
                >
                  <PlayCircle size={9} />
                  passo {step.index} · {formatDuration(step.duration_ms)}
                  <span className="text-zinc-500">
                    {formatNumber(step.tokens_input + step.tokens_output)}t
                  </span>
                </span>
              ))}
            </div>
          ) : null}

          {timeline?.thinking.text?.trim() ? (
          <Band
            title="Raciocínio"
            icon={<BrainIcon />}
            tooltip="Thinking do modelo"
            defaultOpen={false}
            startedAt={timeline?.thinking.started_at}
            endedAt={timeline?.thinking.ended_at}
            durationMs={timeline?.thinking.duration_ms ?? 0}
            summary={
              timeline?.thinking.text?.trim().split("\n")[0] ||
              "O modelo não expôs raciocínio nesta execução."
            }
            tone="purple"
          >
            <pre className="whitespace-pre-wrap break-words font-mono text-[10px] leading-relaxed text-zinc-400">
              {timeline?.thinking.text || "Sem conteúdo de raciocínio."}
            </pre>
            {timeline?.thinking.truncated ? (
              <p className="mt-2 text-[9px] text-amber-300">
                Raciocínio truncado para manter a interface rápida.
              </p>
            ) : null}
          </Band>
          ) : null}

          {timeline?.executed.tools.length ? (
          <Band
            title="Executado"
            icon={<Terminal size={14} />}
            tooltip="Ferramentas e comandos"
            defaultOpen={false}
            startedAt={timeline?.executed.started_at}
            endedAt={timeline?.executed.ended_at}
            durationMs={timeline?.executed.duration_ms ?? 0}
            summary={
              timeline?.executed.tools.length
                ? `${timeline.executed.tools.length} ferramenta(s) · ${formatDuration(
                    timeline.executed.duration_ms,
                  )}`
                : "Nenhuma ferramenta executada."
            }
            tone="blue"
          >
            {timeline?.executed.tools.length ? (
              <div className="space-y-1.5">
                {timeline.executed.tools.map((tool) => (
                  <ToolRow
                    key={tool.id}
                    tool={tool}
                    workspaceRoot={timeline.workspace_root}
                  />
                ))}
              </div>
            ) : (
              <p className="text-[10px] text-zinc-500">
                Nenhuma ferramenta registrada.
              </p>
            )}
          </Band>
          ) : null}

          <div
            className={cn(
              "rounded-xl border",
              failedRun
                ? "border-red-500/20 bg-red-500/[0.035]"
                : "border-emerald-500/20 bg-emerald-500/[0.035]",
            )}
          >
            <div
              className={cn(
                "flex flex-wrap items-center gap-2 border-b px-3.5 py-2.5",
                failedRun ? "border-red-500/15" : "border-emerald-500/15",
              )}
            >
              {failedRun ? (
                <XCircle size={14} className="text-red-400" />
              ) : (
                <CheckCircle2 size={14} className="text-emerald-400" />
              )}
              <span
                className={cn(
                  "text-[11px] font-semibold",
                  failedRun ? "text-red-100" : "text-emerald-100",
                )}
              >
                Resultado
              </span>
              <span className="font-mono text-[9px] text-zinc-500">
                {formatDuration(timeline?.result.duration_ms ?? 0)}
              </span>
              <span className="font-mono text-[9px] text-zinc-500">
                {formatClock(timeline?.result.started_at)} →{" "}
                {formatClock(timeline?.result.ended_at)}
              </span>
              <Button
                variant="ghost"
                size="sm"
                className="ml-auto h-6"
                onClick={() => void copy(timeline?.result.text ?? summary ?? "")}
              >
                <Copy size={11} />
                copiar
              </Button>
            </div>
            <div className="px-3.5 py-3">
              <pre className="whitespace-pre-wrap break-words font-sans text-[11px] leading-relaxed text-zinc-200">
                {timeline?.result.text?.trim() ||
                  summary ||
                  "Sem texto final. Veja as ferramentas executadas e o modo bruto."}
              </pre>
              {timeline?.result.truncated ? (
                <p className="mt-2 text-[9px] text-amber-300">
                  Resultado truncado. Use o modo bruto para ver o log completo.
                </p>
              ) : null}
            </div>
          </div>

          {error ? (
            <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
              {error}
            </p>
          ) : null}
            </>
          )}
          </div>
          {timeline?.run?.task_id ? (
            <div className="shrink-0 border-t border-zinc-800/70 bg-zinc-950/80 p-3">
              <p className="mb-2 text-[9px] text-zinc-500">
                Último agente/modelo:{" "}
                <span className="font-mono text-zinc-400">
                  @{run?.agent_name ?? "router"} · {run?.model_id ?? "modelo"}
                </span>
              </p>
              <div className="mb-2 grid gap-2 sm:grid-cols-2">
                <AgentCombobox
                  value={composerAgent}
                  onChange={(value) => setComposerAgent(value)}
                  workspaceId={timeline.run.workspace_id}
                  placeholder={`automático (router)${run?.agent_name ? ` · @${run.agent_name}` : ""}`}
                />
                <ModelCombobox
                  value={composerModel}
                  onChange={setComposerModel}
                  placeholder="modelo automático"
                  routerHint={`Modelo automático. Último usado: ${
                    run?.model_id ?? "desconhecido"
                  }.`}
                />
              </div>
              <div className="flex items-stretch gap-2">
                <SlashTextarea
                  value={composer}
                  onChange={setComposer}
                  workspaceId={timeline.run.workspace_id}
                  rows={3}
                  submitOnEnter
                  onSubmit={() => void sendComment()}
                  placeholder="Responder, comentar ou pedir uma continuação… (Enter envia)"
                  className="min-h-[88px] flex-1"
                />
                <Button
                  size="sm"
                  className="h-auto w-20 shrink-0 flex-col gap-1"
                  onClick={() => void sendComment()}
                  disabled={sending || !composer.trim()}
                  title="Enviar mensagem e continuar a mesma execução"
                >
                  {sending ? (
                    <Loader2 size={15} className="animate-spin" />
                  ) : (
                    <Send size={15} />
                  )}
                  Enviar
                </Button>
              </div>
              <div className="mt-2 flex flex-wrap items-center gap-2">
                <label className="flex items-center gap-2 text-[11px] text-zinc-400">
                  <input
                    type="checkbox"
                    checked={continueExecution}
                    onChange={(event) => setContinueExecution(event.target.checked)}
                    className="accent-blue-500"
                  />
                  continuar na mesma execução
                </label>
                <span className="text-[9px] text-zinc-500">
                  A continuação vira uma tentativa interna; o histórico continua
                  no mesmo run.
                </span>
              </div>
            </div>
          ) : null}
        </div>
      )}

      <ForkTaskModal
        open={!!forkSource}
        source={forkSource}
        runId={runId}
        onClose={() => setForkSource(null)}
        onSaved={async () => {
          setForkSource(null);
          toast({ tone: "success", title: "Fork criado a partir deste run" });
        }}
      />
    </div>
  );
}
