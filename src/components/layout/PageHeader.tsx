import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export function PageHeader({
  title,
  description,
  actions,
  className,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-wrap items-start justify-between gap-3",
        className,
      )}
    >
      <div className="min-w-0 flex-1">
        <h1 className="type-page">{title}</h1>
        {description ? (
          <p className="mt-1 max-w-prose text-[13.5px] leading-relaxed text-zinc-400">
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
    <div className="mx-auto min-w-0 w-full max-w-[1440px] space-y-5 p-5 pb-10 lg:p-7 lg:pb-12">
      {children}
    </div>
  );
}

/** Full-height page where only explicit child scrollers should scroll. */
export function PageBodyFill({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col gap-4 overflow-hidden p-5 lg:p-7">
      {children}
    </div>
  );
}
