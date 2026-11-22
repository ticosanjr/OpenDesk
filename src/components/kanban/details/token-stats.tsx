import { formatNumber, formatUsd } from "@/lib/format";
import type { TaskRun } from "@/lib/types";

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
      <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">{label}</p>
      <p className="mt-0.5 font-mono text-[13.5px] text-zinc-200">{value}</p>
    </div>
  );
}

export function TaskTokenTotals({ runs }: { runs: TaskRun[] }) {
  if (runs.length === 0) return null;
  const runTotals = runs.reduce(
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
    <div className="rounded-lg border border-zinc-800/80 bg-zinc-950/40 px-3 py-2.5">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-[12px] font-medium uppercase tracking-wide text-zinc-500">
          Tokens desta tarefa
        </span>
        <span className="font-mono text-[12px] text-zinc-500">
          {runs.length} execução(ões) · {formatUsd(runTotals.cost)}
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
  );
}
