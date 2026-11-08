import { useState, type ReactNode } from "react";
import { ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";

export function CollapsibleSection({
  title,
  summary,
  defaultOpen = false,
  children,
  className,
}: {
  title: string;
  summary?: string;
  defaultOpen?: boolean;
  children: ReactNode;
  className?: string;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section
      className={cn(
        "rounded-xl border border-zinc-800/70 bg-zinc-950/30",
        className,
      )}
    >
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="flex w-full items-center gap-2 px-3 py-2.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60"
      >
        <ChevronDown
          size={14}
          className={cn(
            "shrink-0 text-zinc-500 transition-transform",
            open ? "rotate-180" : undefined,
          )}
        />
        <span className="text-[11px] font-medium text-zinc-300">{title}</span>
        {summary && !open ? (
          <span className="ml-auto min-w-0 truncate text-[9px] text-zinc-500">
            {summary}
          </span>
        ) : null}
      </button>
      {open ? (
        <div className="border-t border-zinc-800/70 p-3">{children}</div>
      ) : null}
    </section>
  );
}
