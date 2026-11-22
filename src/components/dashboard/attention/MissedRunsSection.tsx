import { Button } from "@/components/ui/button";
import { Section } from "@/components/dashboard/attention/Section";
import type { Attention } from "@/components/dashboard/attention/types";

export function MissedRunsSection({
  runs,
  busy,
  onRunAll,
  onIgnoreAll,
  onRun,
  onIgnore,
}: {
  runs: Attention["missed_runs"];
  busy: string | null;
  onRunAll: () => void;
  onIgnoreAll: () => void;
  onRun: (id: string) => void;
  onIgnore: (id: string) => void;
}) {
  if (runs.length === 0) return null;
  return (
    <Section title="Execuções perdidas" count={runs.length} tone="amber">
      <div className="space-y-2">
        <div className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-amber-500/20 bg-amber-500/[0.04] px-3 py-2">
          <p className="text-[11.5px] text-amber-100">
            O app/engine estava offline ou o horário passou. Escolha o
            que fazer; o padrão é perguntar.
          </p>
          <div className="flex gap-1.5">
            <Button
              size="xs"
              disabled={busy === "missed-all"}
              onClick={onRunAll}
            >
              Executar todas
            </Button>
            <Button
              size="xs"
              variant="outline"
              disabled={busy === "missed-all"}
              onClick={onIgnoreAll}
            >
              Ignorar todas
            </Button>
          </div>
        </div>
        {runs.map((missed) => (
          <div
            key={missed.id}
            className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
          >
            <div className="min-w-0 flex-1">
              <p className="truncate text-[12.5px] font-medium text-zinc-200">
                {missed.schedule_name ?? missed.schedule_id}
              </p>
              <p className="font-mono text-[13px] text-zinc-500">
                {new Date(missed.scheduled_for).toLocaleString("pt-BR")} ·{" "}
                {missed.reason === "OFFLINE" ? "offline" : "atraso"}
              </p>
            </div>
            <Button
              size="xs"
              disabled={busy === `missed-${missed.id}`}
              onClick={() => onRun(missed.id)}
            >
              Executar
            </Button>
            <Button
              size="xs"
              variant="outline"
              disabled={busy === `missed-${missed.id}`}
              onClick={() => onIgnore(missed.id)}
            >
              Ignorar
            </Button>
          </div>
        ))}
      </div>
    </Section>
  );
}
