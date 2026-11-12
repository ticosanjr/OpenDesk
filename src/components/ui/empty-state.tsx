import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  className,
  compact = false,
}: {
  icon?: LucideIcon;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
  /** Use inside a card so the empty state stays under ~160px tall. */
  compact?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center rounded-xl border border-dashed border-zinc-800/80 bg-zinc-950/30 text-center",
        compact ? "px-4 py-6" : "px-6 py-10",
        className,
      )}
    >
      {Icon ? (
        <div
          className={cn(
            "flex items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/60 text-zinc-400",
            compact ? "mb-2.5 h-9 w-9" : "mb-3 h-11 w-11",
          )}
        >
          <Icon size={compact ? 17 : 20} strokeWidth={1.6} />
        </div>
      ) : null}
      <p
        className={cn(
          "font-semibold text-zinc-200",
          compact ? "text-[13px]" : "text-[14px]",
        )}
      >
        {title}
      </p>
      {description ? (
        <p
          className={cn(
            "max-w-sm leading-relaxed text-zinc-400",
            compact ? "mt-1 text-[12px]" : "mt-1.5 text-[12.5px]",
          )}
        >
          {description}
        </p>
      ) : null}
      {action ? <div className={compact ? "mt-3" : "mt-4"}>{action}</div> : null}
    </div>
  );
}
