import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  BrainCircuit,
  Braces,
  CheckCircle2,
  ChevronRight,
  Clock3,
  Copy,
  ExternalLink,
  FileText,
  FolderOpen,
  Loader2,
  MessageSquare,
  PlayCircle,
  RefreshCw,
  Send,
  Terminal,
  User,
  Wrench,
  XCircle,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { apiGet, apiPost, apiText } from "@/lib/api";
import type {
  RunTimeline,
  TaskMessage,
  TimelineEvent,
  TimelineTool,
} from "@/lib/types";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { WorkingIndicator } from "@/components/ui/working-indicator";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EntityLink } from "@/components/ui/entity-link";
import { EmptyState } from "@/components/ui/empty-state";
import { links } from "@/lib/links";
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
  const [composerModel, setComposerModel] = useState("");
  const [continueExecution, setContinueExecution] = useState(true);
  const [sending, setSending] = useState(false);
  const [follow, setFollow] = useState(false);

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
        `/api/runs/${encodeURIComponent(runId)}/timeline`,
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
        model: composerModel.trim() || null,
      });
      setComposer("");
      toast({
        tone: "success",
        title: continueExecution ? "Mensagem enviada; nova tentativa enfileirada" : "Comentário adicionado",
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
      await apiPost(`/api/tasks/${taskId}/memory`, { content: text });
      toast({ tone: "success", title: "Fixado na memória do workspace" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível fixar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function forkHere() {
    const taskId = timeline?.run?.task_id;
    if (!taskId) return;
    try {
      const forked = await apiPost<{ id: string; title: string }>(
        `/api/runs/${runId}/fork`,
        {},
      );
      toast({
        tone: "success",
        title: "Fork criado a partir deste run",
        description: forked.title,
        href: links.task(forked.id),
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível criar o fork",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function openOutputFolder() {
    const path = timeline?.output_directory;
    if (!path) return;
    try {
      await openPath(path);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível abrir a pasta",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
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
  const totalTokens =
    (run?.tokens_input ?? 0) + (run?.tokens_output ?? 0);

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
          <Badge variant={statusVariant(status)}>{status}</Badge>
          {status === "RUNNING" ? <WorkingIndicator label="executando" /> : null}
          {timeline?.status === "RUNNING" || timeline?.log_missing === false ? (
            <span className="inline-flex items-center gap-1 font-mono text-[9px] text-zinc-600">
              <span
                className={cn(
                  "h-1.5 w-1.5 rounded-full",
                  status === "RUNNING" ? "animate-pulse bg-blue-400" : "bg-emerald-400",
                )}
              />
              {status === "RUNNING" ? "ao vivo" : "encerrado"}
            </span>
          ) : null}
          <span className="font-mono text-[10px] text-zinc-500">
            {formatDuration(timeline?.duration_ms ?? run?.duration_ms)}
          </span>
          {timeline?.steps.length ? (
            <Badge variant="default">{timeline.steps.length} passos</Badge>
          ) : null}
          <span className="ml-auto flex flex-wrap items-center gap-1.5">
            {run?.task_id ? (
              <EntityLink to={links.task(run.task_id)} chip>
                <FileText size={10} />
                abrir tarefa
              </EntityLink>
            ) : null}
            {timeline?.artifacts.length ? (
              <EntityLink to={`/artifacts?run=${encodeURIComponent(runId)}`} chip>
                <FileText size={10} />
                artefatos ({timeline.artifacts.length})
              </EntityLink>
            ) : null}
            {timeline?.output_directory ? (
              <>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7"
                  onClick={() => void openOutputFolder()}
                  title={timeline.output_directory}
                >
                  <FolderOpen size={12} />
                  pasta
                </Button>
                {timeline.output_directory_relative ? (
                  <EntityLink to={links.file(timeline.output_directory_relative)} chip>
                    explorador
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
                  title="Adicionar o resultado à MEMORY.md do workspace"
                >
                  memória
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7"
                  onClick={() => void forkHere()}
                  title="Criar uma tarefa independente com os artefatos deste run"
                >
                  fork daqui
                </Button>
              </>
            ) : null}
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={() => void copy(summary ?? "")}
            >
              <Copy size={12} />
              copiar
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={() => setView(view === "stream" ? "bands" : "stream")}
              title="Alternar entre conversa intercalada e faixas"
            >
              {view === "stream" ? "bandas" : "conversa"}
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
              onClick={() => void load()}
              title="Atualizar"
            >
              <RefreshCw size={12} className={loading ? "animate-spin" : undefined} />
            </Button>
          </span>
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 font-mono text-[9px] text-zinc-600">
          <span>{run?.agent_name ?? "agente"}</span>
          <span>{run?.model_id ?? "modelo"}</span>
          {run ? (
            <>
              <span>{formatNumber(totalTokens)} tokens</span>
              <span>{formatUsd(run.estimated_cost_usd)}</span>
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
          <span className="mr-1 font-mono text-[9px] uppercase tracking-wide text-zinc-600">
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
          {view === "stream" ? (
            <StreamView
              timeline={timeline}
              messages={messages}
              workspaceRoot={timeline?.workspace_root}
              summary={summary}
              failedRun={failedRun}
              copy={copy}
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
                  <span className="text-zinc-700">
                    {formatNumber(step.tokens_input + step.tokens_output)}t
                  </span>
                </span>
              ))}
            </div>
          ) : null}

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
              <p className="text-[10px] text-zinc-600">
                Nenhuma ferramenta registrada.
              </p>
            )}
          </Band>

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
              <span className="font-mono text-[9px] text-zinc-600">
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
              <div className="mb-2 grid gap-2 sm:grid-cols-2">
                <AgentCombobox
                  value={composerAgent}
                  onChange={(value) => setComposerAgent(value)}
                  workspaceId={timeline.run.workspace_id}
                  placeholder="agente automático"
                />
                <ModelCombobox
                  value={composerModel}
                  onChange={setComposerModel}
                  placeholder="modelo automático"
                />
              </div>
              <SlashTextarea
                value={composer}
                onChange={setComposer}
                workspaceId={timeline.run.workspace_id}
                rows={2}
                submitOnEnter
                onSubmit={() => void sendComment()}
                placeholder="Responder, comentar ou pedir uma continuação… (Enter envia)"
                className="min-h-[48px]"
              />
              <div className="mt-2 flex flex-wrap items-center gap-2">
                <label className="flex items-center gap-2 text-[11px] text-zinc-400">
                  <input
                    type="checkbox"
                    checked={continueExecution}
                    onChange={(event) => setContinueExecution(event.target.checked)}
                    className="accent-blue-500"
                  />
                  continuar execução (nova tentativa)
                </label>
                <Button
                  size="sm"
                  className="ml-auto h-8"
                  onClick={() => void sendComment()}
                  disabled={sending || !composer.trim()}
                >
                  {sending ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    <Send size={13} />
                  )}
                  Enviar
                </Button>
              </div>
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
}

function StreamView({
  timeline,
  messages,
  workspaceRoot,
  summary,
  failedRun,
  copy,
}: {
  timeline: RunTimeline | null;
  messages: TaskMessage[];
  workspaceRoot?: string | null;
  summary?: string;
  failedRun: boolean;
  copy: (text: string) => Promise<void>;
}) {
  const entries = useMemo(() => {
    const items: Array<{
      at: number;
      event?: TimelineEvent;
      message?: TaskMessage;
    }> = [];
    for (const event of timeline?.events ?? []) {
      items.push({
        at: event.started_at ? new Date(event.started_at).getTime() : 0,
        event,
      });
    }
    for (const message of messages) {
      items.push({ at: new Date(message.created_at).getTime(), message });
    }
    return items.sort((left, right) => left.at - right.at);
  }, [messages, timeline?.events]);

  if (entries.length === 0) {
    return (
      <div className="space-y-2">
        <p className="rounded-lg border border-dashed border-zinc-800 px-4 py-8 text-center text-[12px] text-zinc-500">
          Nenhum evento estruturado. Use “bruto” para ver o JSONL original.
        </p>
        <ResultCard
          text={timeline?.result.text?.trim() ?? ""}
          summary={summary}
          failed={failedRun}
          durationMs={timeline?.result.duration_ms ?? 0}
          startedAt={timeline?.result.started_at}
          endedAt={timeline?.result.ended_at}
          truncated={timeline?.result.truncated ?? false}
          copy={copy}
        />
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {entries.map((entry, index) => {
        if (entry.message) {
          const message = entry.message;
          return (
            <div key={`message-${message.id}`} className="flex justify-end">
              <div className="max-w-[85%] rounded-xl border border-blue-500/25 bg-blue-500/10 px-3 py-2 text-blue-100">
                <div className="mb-1 flex items-center justify-end gap-1 font-mono text-[10px] text-blue-300/70">
                  <User size={10} />
                  você · {formatClock(message.created_at)}
                </div>
                <p className="whitespace-pre-wrap text-[12px] leading-relaxed">
                  {message.content}
                </p>
              </div>
            </div>
          );
        }

        const event = entry.event!;
        if (event.kind === "step") {
          return (
            <div
              key={`step-${event.step_index}-${index}`}
              className="flex items-center gap-2 py-1 font-mono text-[11px] font-semibold uppercase tracking-wide text-zinc-500"
            >
              <PlayCircle size={12} />
              passo {event.step_index} · {formatDuration(event.duration_ms)}
            </div>
          );
        }
        if (event.kind === "thinking") {
          return (
            <Band
              key={`thinking-${index}`}
              title="Raciocínio"
              icon={<BrainIcon />}
              tooltip="Pensamento do modelo"
              defaultOpen={false}
              startedAt={event.started_at}
              endedAt={event.ended_at}
              durationMs={event.duration_ms}
              summary={event.text.trim().split("\n")[0] || "pensamento sem texto"}
              tone="purple"
            >
              <pre className="whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-zinc-300">
                {event.text || "Sem conteúdo de raciocínio."}
              </pre>
            </Band>
          );
        }
        if (event.kind === "tool" && event.tool) {
          return (
            <ToolRow
              key={`tool-${event.tool.id}-${index}`}
              tool={event.tool}
              workspaceRoot={workspaceRoot}
            />
          );
        }
        if (event.kind === "comment") {
          return (
            <div
              key={`comment-${index}`}
              className="rounded-xl border border-zinc-800/70 bg-zinc-900/40 px-3 py-2"
            >
              <div className="mb-1 flex flex-wrap items-center gap-2 font-mono text-[10px] text-zinc-500">
                <MessageSquare size={11} />
                modelo
                <span>
                  {formatClock(event.started_at)} → {formatClock(event.ended_at)}
                </span>
                <span>{formatDuration(event.duration_ms)}</span>
              </div>
              <p className="whitespace-pre-wrap text-[12px] leading-relaxed text-zinc-200">
                {event.text}
              </p>
            </div>
          );
        }
        return null;
      })}
      <ResultCard
        text={timeline?.result.text?.trim() ?? ""}
        summary={summary}
        failed={failedRun}
        durationMs={timeline?.result.duration_ms ?? 0}
        startedAt={timeline?.result.started_at}
        endedAt={timeline?.result.ended_at}
        truncated={timeline?.result.truncated ?? false}
        copy={copy}
      />
    </div>
  );
}

function ResultCard({
  text,
  summary,
  failed,
  durationMs,
  startedAt,
  endedAt,
  truncated,
  copy,
}: {
  text: string;
  summary?: string;
  failed: boolean;
  durationMs: number;
  startedAt?: string | null;
  endedAt?: string | null;
  truncated: boolean;
  copy: (text: string) => Promise<void>;
}) {
  return (
    <div
      className={cn(
        "rounded-xl border",
        failed
          ? "border-red-500/20 bg-red-500/[0.035]"
          : "border-emerald-500/20 bg-emerald-500/[0.035]",
      )}
    >
      <div
        className={cn(
          "flex flex-wrap items-center gap-2 border-b px-3.5 py-2.5",
          failed ? "border-red-500/15" : "border-emerald-500/15",
        )}
      >
        {failed ? (
          <XCircle size={14} className="text-red-400" />
        ) : (
          <CheckCircle2 size={14} className="text-emerald-400" />
        )}
        <span
          className={cn(
            "text-[12px] font-semibold",
            failed ? "text-red-100" : "text-emerald-100",
          )}
        >
          Resultado
        </span>
        <span className="font-mono text-[10px] text-zinc-500">
          {formatDuration(durationMs)}
        </span>
        <span className="font-mono text-[10px] text-zinc-600">
          {formatClock(startedAt)} → {formatClock(endedAt)}
        </span>
        <Button
          variant="ghost"
          size="sm"
          className="ml-auto h-6"
          onClick={() => void copy(text || summary || "")}
        >
          <Copy size={11} />
          copiar
        </Button>
      </div>
      <div className="px-3.5 py-3">
        <pre className="whitespace-pre-wrap break-words font-sans text-[12px] leading-relaxed text-zinc-100">
          {text || summary || "Sem texto final. Veja as ferramentas executadas e o modo bruto."}
        </pre>
        {truncated ? (
          <p className="mt-2 text-[10px] text-amber-300">
            Resultado truncado. Use o modo bruto para ver o log completo.
          </p>
        ) : null}
      </div>
    </div>
  );
}

function Band({
  title,
  icon,
  tooltip,
  defaultOpen,
  startedAt,
  endedAt,
  durationMs,
  summary,
  tone,
  children,
}: {
  title: string;
  icon: ReactNode;
  tooltip?: string;
  defaultOpen: boolean;
  startedAt?: string | null;
  endedAt?: string | null;
  durationMs: number;
  summary: string;
  tone: "purple" | "blue";
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div
      className={cn(
        "overflow-hidden rounded-xl border",
        tone === "purple"
          ? "border-violet-500/20 bg-violet-500/[0.035]"
          : "border-blue-500/20 bg-blue-500/[0.035]",
      )}
    >
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
        className="flex w-full flex-wrap items-center gap-2 px-3.5 py-2.5 text-left hover:bg-white/[0.02]"
        title={tooltip}
      >
        <ChevronRight
          size={13}
          className={cn(
            "shrink-0 text-zinc-500 transition-transform",
            open && "rotate-90",
          )}
        />
        <span className="shrink-0">{icon}</span>
        <span
          className={cn(
            "text-[11px] font-semibold",
            tone === "purple" ? "text-violet-100" : "text-blue-100",
          )}
        >
          {title}
        </span>
        <span className="font-mono text-[9px] text-zinc-500">
          {formatDuration(durationMs)}
        </span>
        <span className="hidden font-mono text-[9px] text-zinc-600 sm:inline">
          {formatClock(startedAt)} → {formatClock(endedAt)}
        </span>
        <span className="min-w-0 flex-1 truncate text-right text-[9px] text-zinc-600">
          {summary}
        </span>
      </button>
      {open ? <div className="border-t border-zinc-800/60 px-3.5 py-3">{children}</div> : null}
    </div>
  );
}

function ToolRow({
  tool,
  workspaceRoot,
}: {
  tool: TimelineTool;
  workspaceRoot?: string | null;
}) {
  const [open, setOpen] = useState(false);
  const path = useMemo(
    () => relativeToolPath(toolPath(tool.input), workspaceRoot),
    [tool.input, workspaceRoot],
  );
  const failed = tool.status === "failed";
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40">
      <button
        type="button"
        onClick={() => setOpen((current) => !current)}
        className="flex w-full flex-wrap items-center gap-2 px-2.5 py-1.5 text-left hover:bg-zinc-900/60"
        aria-expanded={open}
      >
        {failed ? (
          <XCircle size={12} className="shrink-0 text-red-400" />
        ) : (
          <Wrench size={12} className="shrink-0 text-zinc-500" />
        )}
        <span className="font-mono text-[10px] text-zinc-200">{tool.summary}</span>
        <Badge variant={failed ? "red" : "green"}>{tool.status}</Badge>
        <span className="ml-auto flex shrink-0 items-center gap-2 font-mono text-[9px] text-zinc-600">
          <Clock3 size={9} />
          {formatClock(tool.started_at)}
          <span>{formatDuration(tool.duration_ms)}</span>
        </span>
      </button>
      {open ? (
        <div className="space-y-2 border-t border-zinc-800/60 px-2.5 py-2">
          <div className="flex flex-wrap items-center gap-2">
            {path ? (
              <EntityLink to={links.file(path)} chip>
                <ExternalLink size={9} />
                abrir {path.split("/").pop()}
              </EntityLink>
            ) : null}
            <span className="font-mono text-[9px] text-zinc-600">
              {formatClock(tool.started_at)} → {formatClock(tool.ended_at)}
            </span>
          </div>
          <div>
            <p className="mb-1 font-mono text-[8px] uppercase tracking-wide text-zinc-600">
              entrada
            </p>
            <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words rounded border border-zinc-800/60 bg-zinc-950 p-2 font-mono text-[9px] text-zinc-400">
              {pretty(tool.input)}
            </pre>
          </div>
          <div>
            <p className="mb-1 font-mono text-[8px] uppercase tracking-wide text-zinc-600">
              saída
            </p>
            <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded border border-zinc-800/60 bg-zinc-950 p-2 font-mono text-[9px] text-zinc-400">
              {tool.output || "sem saída"}
            </pre>
            {tool.output_truncated ? (
              <p className="mt-1 text-[9px] text-amber-300">
                Saída truncada.
              </p>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function relativeToolPath(path: string | null, root?: string | null): string | null {
  if (!path) return null;
  if (!path.startsWith("/") && !path.includes(":\\")) return path;
  if (root) {
    const normalizedRoot = root.replace(/\/$/, "");
    if (path.startsWith(normalizedRoot + "/")) {
      return path.slice(normalizedRoot.length + 1);
    }
  }
  return null;
}

function toolPath(input: string): string | null {
  try {
    const value = JSON.parse(input) as Record<string, unknown>;
    const path = value.path ?? value.filePath ?? value.file_path;
    return typeof path === "string" ? path : null;
  } catch {
    return null;
  }
}

function pretty(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

function BrainIcon() {
  return <BrainCircuit size={14} className="text-violet-400" />;
}
