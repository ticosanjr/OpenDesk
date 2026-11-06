import { Cog } from "lucide-react";
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
    <Tooltip content="Execução em andamento — as engrenagens param quando o run termina">
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-md border border-blue-500/25 bg-blue-500/10 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-blue-200",
        className,
      )}
      title="Execução em andamento"
    >
      <Cog size={11} className="opendesk-gear" />
      <Cog size={9} className="opendesk-gear-reverse text-blue-300/80" />
      {label}
    </span>
    </Tooltip>
  );
}
