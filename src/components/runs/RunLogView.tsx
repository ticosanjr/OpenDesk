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
  Loader2,
  PlayCircle,
  RefreshCw,
  Terminal,
  Wrench,
  XCircle,
} from "lucide-react";
import { apiGet, apiText } from "@/lib/api";
import type { RunTimeline, TimelineTool } from "@/lib/types";
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
  const [timeline, setTimeline] = useState<RunTimeline | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [raw, setRaw] = useState<string | null>(null);
  const [rawLoading, setRawLoading] = useState(false);
  const [showRaw, setShowRaw] = useState(false);

  const load = useCallback(async () => {
    try {
      const value = await apiGet<RunTimeline>(
        `/api/runs/${encodeURIComponent(runId)}/timeline`,
      );
      setTimeline(value);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [runId]);

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

  const running =
    timeline?.status === "RUNNING" ||
    timeline?.run?.status === "RUNNING" ||
    loading;

  useEffect(() => {
    if (!running) return;
    const handle = window.setInterval(() => void load(), 2500);
    return () => window.clearInterval(handle);
  }, [load, running]);

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
    <div className={cn("flex min-h-0 flex-col", compact ? "" : "gap-3")}>
      <div className="border-b border-zinc-800/70 px-4 py-3">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant={statusVariant(status)}>{status}</Badge>
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
      </div>

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
        <div className="min-h-0 flex-1 space-y-2 overflow-auto p-3">
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
        </div>
      )}
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
