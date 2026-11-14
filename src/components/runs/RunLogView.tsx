import { useCallback, useEffect, useState } from "react";
import { ArrowDown, Loader2, RefreshCw } from "lucide-react";
import { openWorkspacePath } from "@/lib/open-path";
import { apiDelete, apiGet, apiPost, apiText } from "@/lib/api";
import type {
  RunTimeline,
  Task,
  TaskInteraction,
  TaskMessage,
} from "@/lib/types";
import { Button } from "@/components/ui/button";
import { ForkTaskModal } from "@/components/kanban/task-form-modal";
import {
  InteractionCard,
  PermissionRequestCard,
} from "@/components/interactions/InteractionCard";
import { links } from "@/lib/links";
import { toast } from "@/store/toast-store";
import { useAppStore } from "@/store/app-store";
import { cn } from "@/lib/utils";
import { RunLogHeader } from "@/components/runs/RunLogHeader";
import {
  StreamView,
  type RunEventFilter,
} from "@/components/runs/run-log-parts";
import { RunLogBands } from "@/components/runs/RunLogBands";
import { RunLogComposer } from "@/components/runs/RunLogComposer";
import { RunInputsStrip } from "@/components/runs/RunInputsStrip";
import { useStickToBottom } from "@/lib/use-stick-to-bottom";

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
  const [filter, setFilter] = useState<RunEventFilter>("all");
  const [bandsOpen, setBandsOpen] = useState(false);
  const [bandSignal, setBandSignal] = useState(0);
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
  const {
    ref: transcriptRef,
    atBottom: transcriptAtBottom,
    onScroll: onTranscriptScroll,
    scrollToBottom: scrollTranscript,
  } = useStickToBottom<HTMLDivElement>(
    `${timeline?.events.length ?? 0}:${timeline?.events_count ?? 0}:${messages.length}:${interactions.length}:${permissions.length}:${view}:${filter}`,
  );

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
    setFilter("all");
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
      await openWorkspacePath(path);
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
      <div className="flex items-center gap-2 p-6 text-[13px] text-zinc-500">
        <Loader2 size={14} className="animate-spin" />
        Carregando execução…
      </div>
    );
  }

  if (error && !timeline) {
    return (
      <div className="space-y-3 p-4">
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
        <Button
          size="sm"
          variant="outline"
          onClick={() => void refresh()}
          disabled={refreshing}
        >
          <RefreshCw
            size={13}
            className={refreshing ? "animate-spin" : undefined}
          />
          Tentar novamente
        </Button>
      </div>
    );
  }

  return (
    <div className={cn("flex h-full min-h-0 flex-col", compact ? "" : "gap-3")}>
      <RunLogHeader
        runId={runId}
        timeline={timeline}
        status={status}
        totals={{
          input: totalInput,
          output: totalOutput,
          tokens: totalTokens,
          cost: totalCost,
        }}
        attemptsCount={attempts.length}
        stepsCount={timeline?.steps.length ?? 0}
        view={view}
        filter={filter}
        bandsOpen={bandsOpen}
        showRaw={showRaw}
        refreshing={refreshing}
        openingFolder={openingFolder}
        forkLoading={forkLoading}
        onToggleView={() => setView(view === "stream" ? "bands" : "stream")}
        onFilterChange={setFilter}
        onToggleBands={() => {
          setBandsOpen((current) => !current);
          setBandSignal((current) => current + 1);
        }}
        onToggleRaw={() => {
          setShowRaw((current) => !current);
          if (!raw && !showRaw) void loadRaw();
        }}
        onRefresh={() => void refresh()}
        onOpenFolder={() => void openOutputFolder()}
        onPinMemory={() => void pinMemory()}
        onFork={() => void forkHere()}
        onCopyExecution={copyExecution}
      />

      <RunInputsStrip inputs={timeline?.inputs ?? []} />

      {showRaw ? (
        <div className="min-h-0 flex-1 overflow-auto p-4">
          {rawLoading ? (
            <p className="flex items-center gap-2 text-[12px] text-zinc-500">
              <Loader2 size={12} className="animate-spin" />
              Carregando log bruto…
            </p>
          ) : (
            <pre className="whitespace-pre-wrap break-words font-mono text-[11.5px] leading-relaxed text-zinc-400">
              {raw || "Log bruto indisponível."}
            </pre>
          )}
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="relative min-h-0 flex-1">
          <div
            ref={transcriptRef}
            onScroll={onTranscriptScroll}
            className="h-full space-y-2 overflow-auto p-3"
          >
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
                filter={filter}
                bandForce={{ signal: bandSignal, open: bandsOpen }}
              />
            ) : (
              <RunLogBands
                timeline={timeline}
                failedRun={failedRun}
                summary={summary}
                error={error}
                onCopy={(value) => void copy(value)}
              />
            )}
          </div>
          {!transcriptAtBottom ? (
            <Button
              variant="outline"
              size="sm"
              className="absolute bottom-3 right-4 h-8 gap-1.5 rounded-full bg-zinc-950/95 shadow-lg"
              onClick={() => scrollTranscript()}
              title="Ir para as ações mais recentes"
            >
              <ArrowDown size={13} />
              ir para o fim
            </Button>
          ) : null}
          </div>
          {timeline?.run?.task_id ? (
            <RunLogComposer
              workspaceId={timeline.run.workspace_id}
              lastAgent={run?.agent_name}
              lastModel={run?.model_id}
              composer={composer}
              agent={composerAgent}
              model={composerModel}
              continueExecution={continueExecution}
              sending={sending}
              onComposerChange={setComposer}
              onAgentChange={setComposerAgent}
              onModelChange={setComposerModel}
              onContinueChange={setContinueExecution}
              onSend={() => void sendComment()}
            />
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
