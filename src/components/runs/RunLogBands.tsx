import { FileText, PlayCircle, Terminal } from "lucide-react";
import { EmptyState } from "@/components/ui/empty-state";
import { formatDuration, formatNumber } from "@/lib/format";
import { Band, BrainIcon, ToolRow } from "@/components/runs/run-log-parts";
import { RunLogResult } from "@/components/runs/RunLogResult";
import type { RunTimeline } from "@/lib/types";

export function RunLogBands({
  timeline,
  failedRun,
  summary,
  error,
  onCopy,
}: {
  timeline: RunTimeline | null;
  failedRun: boolean;
  summary: string | null | undefined;
  error: string | null;
  onCopy: (text: string) => void;
}) {
  return (
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
              className="inline-flex items-center gap-1 rounded-md border border-zinc-800/70 bg-zinc-950/40 px-1.5 py-1 font-mono text-[11.5px] text-zinc-500"
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
          <pre className="whitespace-pre-wrap break-words font-mono text-[12px] leading-relaxed text-zinc-400">
            {timeline?.thinking.text || "Sem conteúdo de raciocínio."}
          </pre>
          {timeline?.thinking.truncated ? (
            <p className="mt-2 text-[11.5px] text-amber-300">
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
            <p className="text-[12px] text-zinc-500">Nenhuma ferramenta registrada.</p>
          )}
        </Band>
      ) : null}

      <RunLogResult
        timeline={timeline}
        summary={summary}
        failedRun={failedRun}
        onCopy={onCopy}
      />

      {error ? (
        <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-200">
          {error}
        </p>
      ) : null}
    </>
  );
}
