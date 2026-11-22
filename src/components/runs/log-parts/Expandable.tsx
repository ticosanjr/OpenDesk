import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

export function Expandable({
  open,
  onToggle,
  header,
  actions,
  children,
  className,
  headerClassName,
  contentClassName,
}: {
  open: boolean;
  onToggle: () => void;
  header: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  headerClassName?: string;
  contentClassName?: string;
}) {
  return (
    <div
      className={cn(
        "group overflow-hidden rounded-2xl border bg-zinc-950/30 transition-[border-color,background-color,box-shadow] duration-200 hover:border-zinc-700/80 hover:bg-white/[0.015]",
        className,
      )}
    >
      <div className="flex items-center gap-2">
        <button
          type="button"
          aria-expanded={open}
          onClick={onToggle}
          className={cn(
            "flex min-w-0 flex-1 flex-wrap items-center gap-2 px-3.5 py-2.5 text-left transition-colors hover:bg-white/[0.02]",
            headerClassName,
          )}
        >
          {header}
        </button>
        {actions ? (
          <div className="flex shrink-0 items-center gap-1 pr-2">{actions}</div>
        ) : null}
      </div>
      <div
        className={cn(
          "grid transition-[grid-template-rows,opacity] duration-200 ease-out",
          open ? "grid-rows-[1fr] opacity-100" : "grid-rows-[0fr] opacity-0",
        )}
        aria-hidden={!open}
      >
        <div className="overflow-hidden">
          <div
            onClick={(event) => {
              const target = event.target as HTMLElement;
              if (
                target.closest(
                  "button,a,input,textarea,select,[data-no-collapse]",
                )
              ) {
                return;
              }
              if (open) onToggle();
            }}
            className={cn(
              "border-t border-zinc-800/60 px-3.5 py-3",
              contentClassName,
            )}
          >
            {children}
          </div>
        </div>
      </div>
    </div>
  );
}
