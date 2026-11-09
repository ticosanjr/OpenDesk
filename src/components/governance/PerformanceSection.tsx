import { Activity } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { formatUsd } from "@/lib/governance";
import type { AgentPerformance } from "@/lib/types";
import { SectionTitle } from "./SectionTitle";

export function PerformanceSection({ rows }: { rows: AgentPerformance[] }) {
  return (
    <section className="space-y-2">
      <SectionTitle
        icon={<Activity size={13} />}
        title="Performance"
        hint="últimos 30 dias"
      />
      {rows.length === 0 ? (
        <EmptyState
          icon={Activity}
          title="Sem execuções no período"
          description="A performance aparece quando os agentes começarem a rodar tarefas."
        />
      ) : (
        <Card>
          <div className="overflow-x-auto">
            <table className="w-full text-left text-[10px]">
              <thead className="border-b border-zinc-800 text-[9px] uppercase tracking-wide text-zinc-600">
                <tr>
                  <th className="px-3 py-2">Agente</th>
                  <th className="px-3 py-2">Runs</th>
                  <th className="px-3 py-2">Sucesso</th>
                  <th className="px-3 py-2">Falhas</th>
                  <th className="px-3 py-2">Custo</th>
                  <th className="px-3 py-2">Duração média</th>
                  <th className="px-3 py-2">Tokens</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr
                    key={row.agent_id}
                    className="border-b border-zinc-800/50 last:border-0"
                  >
                    <td className="px-3 py-2 font-mono text-zinc-300">
                      @{row.agent_id}
                    </td>
                    <td className="px-3 py-2 text-zinc-400">{row.runs}</td>
                    <td className="px-3 py-2">
                      <Badge
                        variant={
                          row.success_rate >= 0.8
                            ? "green"
                            : row.success_rate >= 0.5
                              ? "yellow"
                              : "red"
                        }
                      >
                        {Math.round(row.success_rate * 100)}%
                      </Badge>
                    </td>
                    <td className="px-3 py-2 text-zinc-400">
                      {row.failed + row.cancelled}
                    </td>
                    <td className="px-3 py-2 text-zinc-400">
                      {formatUsd(row.cost_usd)}
                    </td>
                    <td className="px-3 py-2 text-zinc-400">
                      {(row.avg_duration_ms / 1000).toFixed(1)}s
                    </td>
                    <td className="px-3 py-2 text-zinc-400">{row.tokens}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      )}
    </section>
  );
}
