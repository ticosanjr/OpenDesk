import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ModelHealth } from "@/lib/types";

export function EngineModelHealth({
  modelHealth,
  onClear,
}: {
  modelHealth: ModelHealth[];
  onClear: (provider?: string, model?: string) => void;
}) {
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
      <div className="flex flex-wrap items-center gap-2">
        <p className="flex-1 text-[12px] uppercase tracking-wide text-zinc-500">
          modelos em cooldown (24h)
        </p>
        {modelHealth.length > 0 ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onClear()}
          >
            limpar todos
          </Button>
        ) : null}
      </div>
      {modelHealth.length === 0 ? (
        <p className="mt-1 text-[12px] text-zinc-500">
          Nenhum modelo bloqueado por região/auth recentemente.
        </p>
      ) : (
        <div className="mt-1 space-y-1">
          {modelHealth.map((item) => (
            <div
              key={`${item.provider_id}/${item.model_id}`}
              className="flex items-center gap-2 text-[12px]"
            >
              <span className="min-w-0 flex-1 truncate font-mono text-zinc-300">
                {item.provider_id}/{item.model_id}
              </span>
              <Badge variant="red">{item.code}</Badge>
              <span className="text-zinc-500">{item.failures}x</span>
              <button
                type="button"
                className="text-[11.5px] text-zinc-500 underline-offset-2 hover:text-zinc-300 hover:underline"
                onClick={() => onClear(item.provider_id, item.model_id)}
              >
                limpar
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
