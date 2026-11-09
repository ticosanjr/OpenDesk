import type { ReactNode } from "react";
import { X } from "lucide-react";
import { Tooltip } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export function Row({
  icon,
  title,
  meta,
  detail,
  actions,
  tone,
}: {
  icon: ReactNode;
  title: ReactNode;
  meta?: string;
  detail?: string | null;
  actions?: ReactNode;
  tone: "amber" | "red" | "violet";
}) {
  const border =
    tone === "red"
      ? "border-red-500/20"
      : tone === "violet"
        ? "border-violet-500/20"
        : "border-amber-500/20";
  const iconTone =
    tone === "red"
      ? "text-red-300"
      : tone === "violet"
        ? "text-violet-300"
        : "text-amber-300";
  return (
    <div
      data-attention-row
      className={cn(
        "grid min-h-[82px] grid-cols-[auto_minmax(0,1fr)_auto] items-start gap-x-2.5 gap-y-1 rounded-lg border bg-zinc-950/40 px-2.5 py-2 sm:h-[82px] sm:grid-cols-[auto_minmax(0,1fr)_250px] sm:items-stretch sm:overflow-hidden",
        border,
      )}
    >
      <span className={cn("mt-0.5 shrink-0", iconTone)}>{icon}</span>
      <div className="min-h-0 min-w-0 sm:overflow-y-auto">
        <p className="line-clamp-1 text-[12px] text-zinc-200">{title}</p>
        {meta ? (
          <p className="truncate font-mono text-[10px] text-zinc-500">{meta}</p>
        ) : null}
        {detail ? (
          <p className="mt-0.5 line-clamp-2 min-h-[26px] break-words text-[10px] leading-snug text-red-200/70">
            {detail}
          </p>
        ) : null}
      </div>
      {actions ? (
        <div className="col-span-3 flex min-h-0 flex-wrap items-center justify-end gap-1.5 sm:col-span-1 sm:w-[250px] sm:max-w-[250px] sm:overflow-y-auto">
          {actions}
        </div>
      ) : null}
    </div>
  );
}

export function DismissButton({
  label,
  onClick,
}: {
  label: string;
  onClick: () => void;
}) {
  return (
    <Tooltip content={label}>
      <button
        type="button"
        aria-label={label}
        onClick={onClick}
        className="rounded p-1 text-zinc-600 transition-colors hover:bg-zinc-800/60 hover:text-zinc-200"
      >
        <X size={12} />
      </button>
    </Tooltip>
  );
}
