import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { Tooltip } from "@/components/ui/tooltip";

export function WorkingIndicator({
  label = "executando",
  className,
}: {
  label?: string;
  className?: string;
}) {
  return (
    <Tooltip content="Execução em andamento — o spinner para quando o run termina">
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-md border border-blue-500/25 bg-blue-500/10 px-1.5 py-0.5 font-mono text-[12px] font-semibold text-blue-200",
        className,
      )}
      title="Execução em andamento"
    >
      <Loader2 size={13} className="animate-spin text-blue-400 shrink-0" />
      {label}
    </span>
    </Tooltip>
  );
}
