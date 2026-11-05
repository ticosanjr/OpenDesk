import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  className,
}: {
  icon?: LucideIcon;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center rounded-xl border border-dashed border-zinc-800/80 bg-zinc-950/30 px-6 py-12 text-center",
        className,
      )}
    >
      {Icon ? (
        <div className="mb-3 flex h-10 w-10 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/60 text-zinc-500">
          <Icon size={18} strokeWidth={1.6} />
        </div>
      ) : null}
      <p className="text-sm font-medium text-zinc-300">{title}</p>
      {description ? (
        <p className="mt-1 max-w-sm text-xs leading-relaxed text-zinc-500">
          {description}
        </p>
      ) : null}
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}
