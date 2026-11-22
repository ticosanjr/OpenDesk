import {
  Activity,
  CircleDollarSign,
  Clock3,
  Database,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { WorkingIndicator } from "@/components/ui/working-indicator";
import type { DashboardStats } from "@/lib/types";
import {
  formatNumber,
  formatPercent,
  formatUsd,
} from "@/lib/format";
import type { LucideIcon } from "lucide-react";
import type { DashboardMetrics } from "@/pages/dashboard/derive";

export function KpiCard({
  icon: Icon,
  label,
  value,
  hint,
  tone = "default",
  active = false,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  hint: string;
  tone?: "default" | "warning";
  active?: boolean;
}) {
  return (
    <div className="bg-zinc-950/25 p-4">
      <div className="flex items-center gap-2">
        <Icon size={15} strokeWidth={1.7} className="shrink-0 text-zinc-400" />
        <p className="min-w-0 flex-1 truncate text-[13px] font-medium uppercase tracking-[0.1em] text-zinc-400">
          {label}
        </p>
        {tone === "warning" ? <Badge variant="yellow">atenção</Badge> : null}
      </div>
      <p className="mt-2 font-mono text-[28px] font-medium leading-tight tracking-[-0.03em] text-zinc-50 [font-variant-numeric:tabular-nums]">
        {value}
      </p>
      <p className="mt-1 text-[13.5px] leading-relaxed text-zinc-400">{hint}</p>
      {active ? (
        <div className="mt-2">
          <WorkingIndicator label="em execução" />
        </div>
      ) : null}
    </div>
  );
}

export function AgentStateBadge({ state }: { state?: string }) {
  const label = state ?? "IDLE";
  const tone =
    label === "WORKING"
      ? "blue"
      : label === "BLOCKED"
        ? "red"
        : label === "DONE"
          ? "green"
          : "default";
  return <Badge variant={tone}>{label.toLowerCase()}</Badge>;
}

export function DashboardKpis({
  dashboard,
  metrics,
}: {
  dashboard: DashboardStats | null;
  metrics: DashboardMetrics;
}) {
  const { spent, dailyLimit, totalSpent, runsTotal, successRate, cacheHit } =
    metrics;
  return (
    <Card className="overflow-hidden">
      <div className="grid gap-px bg-zinc-800/50 sm:grid-cols-2 xl:grid-cols-4">
        <KpiCard
          icon={CircleDollarSign}
          label="Gasto hoje"
          value={formatUsd(spent)}
          hint={`limite diário ${formatUsd(dailyLimit)} · ${formatPercent(
            Math.min(100, (spent / Math.max(dailyLimit, 0.01)) * 100),
          )} usado · total ${formatUsd(totalSpent)}`}
          tone={spent / Math.max(dailyLimit, 0.01) >= 0.8 ? "warning" : "default"}
        />
        <KpiCard
          icon={Activity}
          label="Execuções & sucesso"
          value={`${formatNumber(runsTotal)} execuções`}
          hint={`${formatPercent(successRate)} sucesso · ${
            dashboard?.runs_failed ?? 0
          } falhas`}
        />
        <KpiCard
          icon={Database}
          label="Cache & tokens"
          value={`${formatPercent(cacheHit)} acertos`}
          hint={`${formatNumber(dashboard?.tokens_input)} entrada · ${formatNumber(
            dashboard?.tokens_output,
          )} saída`}
        />
        <KpiCard
          icon={Clock3}
          label="Fila"
          value={`${dashboard?.queue_running ?? 0} rodando`}
          hint={`${dashboard?.queue_waiting ?? 0} aguardando · ${
            dashboard?.artifacts_total ?? 0
          } artefatos · inclui automações`}
          active={(dashboard?.queue_running ?? 0) > 0}
        />
      </div>
    </Card>
  );
}
