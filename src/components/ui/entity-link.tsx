import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { cn } from "@/lib/utils";

export function EntityLink({
  to,
  children,
  icon,
  chip = false,
  title,
  className,
}: {
  to: string;
  children: ReactNode;
  icon?: ReactNode;
  chip?: boolean;
  title?: string;
  className?: string;
}) {
  return (
    <Link
      to={to}
      title={title}
      onClick={(event) => event.stopPropagation()}
      className={cn(
        "inline-flex max-w-full items-center gap-1 rounded align-baseline transition-colors",
        chip
          ? "border border-zinc-800/80 bg-zinc-900/50 px-1.5 py-0.5 font-mono text-[12px] text-zinc-300 hover:border-blue-500/40 hover:text-blue-200"
          : "text-inherit underline decoration-zinc-600/60 underline-offset-2 hover:text-blue-300 hover:decoration-blue-400/70",
        className,
      )}
    >
      {icon}
      <span className="truncate">{children}</span>
    </Link>
  );
}
