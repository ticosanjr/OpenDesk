import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export function PageHeader({
  eyebrow,
  title,
  description,
  actions,
  className,
}: {
  eyebrow?: string;
  title: string;
  description?: string;
  actions?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex flex-wrap items-start justify-between gap-4", className)}>
      <div>
        {eyebrow ? (
          <p className="mb-1 font-mono text-[9px] uppercase tracking-[0.18em] text-zinc-600">
            {eyebrow}
          </p>
        ) : null}
        <h1 className="text-lg font-semibold tracking-tight text-zinc-100">{title}</h1>
        {description ? (
          <p className="mt-1 max-w-2xl text-xs leading-relaxed text-zinc-500">
            {description}
          </p>
        ) : null}
      </div>
      {actions ? <div className="flex items-center gap-2">{actions}</div> : null}
    </div>
  );
}

export function PageBody({ children }: { children: ReactNode }) {
  return <div className="space-y-5 p-5 lg:p-7">{children}</div>;
}
