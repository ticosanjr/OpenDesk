import { useEffect, useState, type ReactNode } from "react";

import { ChevronRight } from "lucide-react";
import { formatClock, formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { Expandable } from "@/components/runs/log-parts/Expandable";

export function Band({
  title,
  icon,
  tooltip,
  defaultOpen,
  startedAt,
  endedAt,
  durationMs,
  summary,
  tone,
  forceOpen,
  children,
}: {
  title: string;
  icon: ReactNode;
  tooltip?: string;
  defaultOpen: boolean;
  startedAt?: string | null;
  endedAt?: string | null;
  durationMs: number;
  summary: string;
  tone: "purple" | "blue";
  /** Header "colapsar/expandir tudo" signal. */
  forceOpen?: { signal: number; open: boolean };
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const forceSignal = forceOpen?.signal ?? 0;
  const forceValue = forceOpen?.open ?? false;
  useEffect(() => {
    if (forceSignal > 0) setOpen(forceValue);
  }, [forceSignal, forceValue]);
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className={cn(
        tone === "purple"
          ? "border-violet-500/20 bg-violet-500/[0.035]"
          : "border-blue-500/20 bg-blue-500/[0.035]",
      )}
      header={
        <>
          <ChevronRight
            size={13}
            className={cn(
              "shrink-0 text-zinc-500 transition-transform duration-200",
              open && "rotate-90",
            )}
          />
          <span className="shrink-0">{icon}</span>
          <span
            className={cn(
              "text-[13px] font-semibold",
              tone === "purple" ? "text-violet-100" : "text-blue-100",
            )}
            title={tooltip}
          >
            {title}
          </span>
          <span className="font-mono text-[11.5px] text-zinc-500">
            {formatDuration(durationMs)}
          </span>
          <span className="hidden font-mono text-[11.5px] text-zinc-500 sm:inline">
            {formatClock(startedAt)} → {formatClock(endedAt)}
          </span>
          <span className="min-w-0 flex-1 truncate text-right text-[11.5px] text-zinc-500">
            {summary}
          </span>
        </>
      }
      contentClassName="px-3.5 py-3"
    >
      {children}
    </Expandable>
  );
}
