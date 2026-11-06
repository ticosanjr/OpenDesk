import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";

export function Tooltip({
  content,
  children,
  className,
}: {
  content: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);

  if (!content) return <>{children}</>;

  return (
    <span
      className={cn("inline-flex", className)}
      onMouseEnter={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        setPosition({ x: rect.left + rect.width / 2, y: rect.top - 8 });
      }}
      onMouseLeave={() => setPosition(null)}
      onFocus={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        setPosition({ x: rect.left + rect.width / 2, y: rect.top - 8 });
      }}
      onBlur={() => setPosition(null)}
    >
      {children}
      {position
        ? createPortal(
            <span
              role="tooltip"
              style={{ left: position.x, top: position.y }}
              className="pointer-events-none fixed z-[80] max-w-xs -translate-x-1/2 -translate-y-full rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-[11px] font-medium leading-snug text-zinc-200 shadow-xl"
            >
              {content}
            </span>,
            document.body,
          )
        : null}
    </span>
  );
}
