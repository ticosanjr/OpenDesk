import type { Dispatch, SetStateAction } from "react";
import { Link } from "react-router-dom";
import { AlertTriangle, Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { OutboundSink } from "@/components/integrations/OutboundSinksCard";
import { formatNumber, formatUsd, relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import type { Task, TaskDiagnostics, TaskRun } from "@/lib/types";
import { cn } from "@/lib/utils";
import { labelClass, runVariant } from "./ui";

export interface TaskContextUsage {
  session_id?: string | null;
  tokens_input: number;
  window: number;
  percent: number;
  messages: number;
}

export function TaskSinksCard({
  sinks,
  selectedSinks,
  setSelectedSinks,
  savingSinks,
  onSave,
}: {
  sinks: OutboundSink[];
  selectedSinks: Set<string>;
  setSelectedSinks: Dispatch<SetStateAction<Set<string>>>;
  savingSinks: boolean;
  onSave: () => void;
}) {
  if (sinks.length === 0) return null;
  return (
    <div className="rounded-lg border border-zinc-800/80 bg-zinc-950/40 px-3 py-2">
      <p className="text-[12px] font-medium uppercase tracking-wide text-zinc-500">
        Entregas automáticas
      </p>
      <div className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1">
        {sinks.map((sink) => (
          <label key={sink.id} className="flex items-center gap-1.5 text-[12px] text-zinc-400">
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
          onClick={onSave}
        >
          {savingSinks ? <Loader2 size={12} className="animate-spin" /> : null}
          Salvar entregas
        </Button>
      </div>
    </div>
  );
}

export function TaskContextCard({
  context,
  compacting,
  onCompact,
}: {
  context: TaskContextUsage;
  compacting: boolean;
  onCompact: () => void;
}) {
  return (
    <div className="rounded-lg border border-zinc-800/80 bg-zinc-950/40 px-3 py-2">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-[12px] font-medium uppercase tracking-wide text-zinc-500">
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
        <span className="font-mono text-[12px] text-zinc-500">
          {formatNumber(context.tokens_input)} / {formatNumber(context.window)} ·{" "}
          {context.percent.toFixed(0)}% · {context.messages} msgs
        </span>
        <Button
          variant="outline"
          size="sm"
          className="h-7"
          disabled={compacting}
          onClick={onCompact}
        >
          {compacting ? <Loader2 size={12} className="animate-spin" /> : null}
          Compactar contexto agora
        </Button>
      </div>
    </div>
  );
}

export function TaskDiagnosticsErrorCard({
  error,
  copied,
  onCopy,
}: {
  error: NonNullable<TaskDiagnostics["error"]>;
  copied: boolean;
  onCopy: () => void;
}) {
  return (
    <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-3 text-red-200">
      <div className="flex flex-wrap items-center gap-2">
        <AlertTriangle size={14} />
        <span className="text-[13px] font-semibold">
          O que falhou: {error.code}
        </span>
        <Badge variant="red">{error.source}</Badge>
        <button
          type="button"
          onClick={onCopy}
          className="ml-auto text-[12px] text-red-300/80 underline-offset-2 hover:underline"
        >
          {copied ? "copiado" : "copiar erro"}
        </button>
      </div>
      <p className="mt-2 whitespace-pre-wrap font-mono text-[12px] leading-relaxed">
        {error.message}
      </p>
      <p className="mt-2 text-[12px] leading-relaxed text-red-100/80">
        <strong>Como resolver:</strong> {error.hint}
      </p>
    </div>
  );
}

export function TaskDiagnosticsErrorNotice({ message }: { message: string }) {
  return (
    <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-200">
      Não foi possível carregar o diagnóstico: {message}
    </p>
  );
}

export function TaskDescriptionSection({ task }: { task: Task }) {
  return (
    <>
      <div>
        <p className={labelClass}>Descrição</p>
        <p className="whitespace-pre-wrap rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 text-[13px] leading-relaxed text-zinc-400">
          {task.description}
        </p>
      </div>
      {task.plan_markdown ? (
        <div>
          <p className={labelClass}>Plano</p>
          <pre className="max-h-[320px] overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 font-mono text-[12px] leading-relaxed text-zinc-300">
            {task.plan_markdown}
          </pre>
        </div>
      ) : null}
    </>
  );
}

export function TaskRecentRuns({ runs }: { runs: TaskRun[] }) {
  return (
    <div>
      <p className={labelClass}>Últimas execuções</p>
      <div className="space-y-1.5">
        {runs.slice(0, 3).map((run) => (
          <Link
            key={run.id}
            to={links.run(run.id)}
            className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[12px] transition-colors hover:border-blue-500/30 hover:bg-zinc-900/50"
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
              <p className="w-full truncate font-mono text-[11.5px] text-red-300/80">
                {run.error_message}
              </p>
            ) : null}
          </Link>
        ))}
      </div>
    </div>
  );
}
