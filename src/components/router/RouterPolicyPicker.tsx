import { Gauge } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import { cn } from "@/lib/utils";

export function RouterPolicyPicker({
  policy,
  onChange,
}: {
  policy: string;
  onChange: (policy: string) => void;
}) {
  return (
    <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
      {Object.entries(ROUTER_POLICY_LABELS).map(([id, label]) => {
        const active = id === policy;
        return (
          <button
            key={id}
            type="button"
            onClick={() => onChange(id)}
            className={cn(
              "rounded-xl border p-4 text-left transition-colors",
              active
                ? "workspace-selected"
                : "border-zinc-800/80 bg-zinc-900/25 hover:border-zinc-700",
            )}
          >
            <div className="flex items-center justify-between">
              <Gauge
                size={15}
                className={active ? "workspace-selected-text" : "text-zinc-500"}
              />
              {active ? <Badge variant="accent">atual</Badge> : null}
            </div>
            <p className="mt-3 text-[13.5px] font-medium text-zinc-200">{label}</p>
            <p className="mt-1 font-mono text-[11.5px] text-zinc-500">{id}</p>
          </button>
        );
      })}
    </div>
  );
}
