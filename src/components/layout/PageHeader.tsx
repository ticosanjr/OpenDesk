import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export function PageHeader({
  title,
  description,
  actions,
  compact = false,
  className,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
  compact?: boolean;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-wrap items-start justify-between gap-3 py-2.5",
        compact ? "min-h-[48px] py-1.5" : "min-h-[64px]",
        className,
      )}
    >
      <div className="min-w-0 flex-1">
        <h1 className="type-page truncate">{title}</h1>
        {description ? (
          <p
            title={description}
            className="mt-0.5 truncate text-[13.5px] leading-relaxed text-zinc-400"
          >
            {description}
          </p>
        ) : null}
      </div>
      {actions ? (
        <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div>
      ) : null}
    </div>
  );
}

export function PageBody({ children }: { children: ReactNode }) {
  return (
    <div className="min-w-0 w-full space-y-5 px-3 py-3 sm:px-4">
      {children}
    </div>
  );
}

/** Full-height page where only explicit child scrollers should scroll. */
export function PageBodyFill({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full min-h-0 w-full min-w-0 flex-1 flex-col gap-4 overflow-hidden px-3 py-3 sm:px-4">
      {children}
    </div>
  );
}
