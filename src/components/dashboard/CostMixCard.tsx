import { useEffect, useState } from "react";
import { CircleDollarSign } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { apiGet } from "@/lib/api";
import { useAppStore } from "@/store/app-store";
import { formatNumber, formatUsd } from "@/lib/format";

interface CostMix {
  days: number;
  models: Array<{
    provider_id: string;
    model_id: string;
    runs: number;
    tokens: number;
    cost_usd: number;
    percent: number;
  }>;
  total_cost_usd: number;
  baseline_cost_usd: number;
  savings_usd: number;
  cache_read: number;
  cache_ratio: number;
}

export function CostMixCard() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const [data, setData] = useState<CostMix | null>(null);

  useEffect(() => {
    if (!workspaceId) return;
    void apiGet<CostMix>(
      `/api/cost/mix?workspace_id=${encodeURIComponent(workspaceId)}&days=7`,
    )
      .then(setData)
      .catch(() => setData(null));
  }, [workspaceId, lastRunEvent?.run_id]);

  if (!data || data.models.length === 0) return null;

  return (
    <Card>
      <CardHeader className="py-3">
        <CardTitle className="flex items-center gap-2 text-[13px]">
          <CircleDollarSign size={15} className="text-zinc-500" />
          Model mix e economia (7 dias)
        </CardTitle>
        <Badge variant="green">economia {formatUsd(data.savings_usd)}</Badge>
      </CardHeader>
      <CardContent className="space-y-2 pt-0">
        <div className="flex h-3 w-full overflow-hidden rounded-full border border-zinc-800 bg-zinc-950">
          {data.models.map((model, index) => (
            <div
              key={`${model.provider_id}/${model.model_id}`}
              title={`${model.provider_id}/${model.model_id} · ${model.percent.toFixed(1)}% · ${formatUsd(model.cost_usd)}`}
              style={{
                width: `${Math.max(model.percent, 1)}%`,
                background: ["#60a5fa", "#a78bfa", "#34d399", "#fbbf24", "#f87171"][
                  index % 5
                ],
              }}
            />
          ))}
        </div>
        <div className="flex flex-wrap gap-x-4 gap-y-1 font-mono text-[10px] text-zinc-500">
          {data.models.slice(0, 6).map((model) => (
            <span key={`${model.provider_id}/${model.model_id}`}>
              {model.model_id} {model.percent.toFixed(1)}%
            </span>
          ))}
        </div>
        <p className="text-[10px] text-zinc-500">
          Gasto real {formatUsd(data.total_cost_usd)} · baseline no modelo mais caro{" "}
          {formatUsd(data.baseline_cost_usd)} · cache hit{" "}
          {(data.cache_ratio * 100).toFixed(1)}% ({formatNumber(data.cache_read)} tokens)
        </p>
      </CardContent>
    </Card>
  );
}
