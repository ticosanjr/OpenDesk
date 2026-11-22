import { Cpu } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { formatNumber, formatUsd } from "@/lib/format";
import type { ModelUsage } from "@/pages/dashboard/derive";

export function ModelUsageCard({ modelUsage }: { modelUsage: ModelUsage[] }) {
  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Cpu size={15} className="text-zinc-500" />
            Uso por modelo
          </CardTitle>
          <CardDescription>
            Chamadas, tokens, custo e latência média por modelo nos runs recentes.
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent>
        {modelUsage.length === 0 ? (
          <EmptyState
            icon={Cpu}
            title="Sem telemetria por modelo ainda"
            description="A tabela é alimentada pelos runs persistidos em task_runs."
            compact
          />
        ) : (
          <div className="scroll-area overflow-auto">
            <table className="w-full text-left">
              <thead className="border-b border-zinc-800/70 text-[11.5px] uppercase tracking-[0.12em] text-zinc-500">
                <tr>
                  <th className="pb-2 font-medium">Modelo</th>
                  <th className="pb-2 text-right font-medium">Chamadas</th>
                  <th className="pb-2 text-right font-medium">Tokens</th>
                  <th className="pb-2 text-right font-medium">Custo</th>
                  <th className="pb-2 text-right font-medium">Latência média</th>
                </tr>
              </thead>
              <tbody>
                {modelUsage.map((model) => (
                  <tr key={model.model} className="border-b border-zinc-800/40 text-[12px]">
                    <td className="max-w-[220px] truncate py-2 font-mono text-zinc-300">
                      {model.model}
                    </td>
                    <td className="py-2 text-right font-mono text-zinc-500">{model.calls}</td>
                    <td className="py-2 text-right font-mono text-zinc-500">
                      {formatNumber(model.tokens)}
                    </td>
                    <td className="py-2 text-right font-mono text-zinc-400">
                      {formatUsd(model.cost)}
                    </td>
                    <td className="py-2 text-right font-mono text-zinc-500">
                      {Math.round(model.duration / Math.max(1, model.calls))} ms
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
