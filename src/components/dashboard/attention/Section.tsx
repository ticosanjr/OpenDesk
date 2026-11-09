import type { ReactNode } from "react";
import { CountPill } from "@/components/ui/count-pill";

export function Section({
  title,
  count,
  tone,
  children,
}: {
  title: string;
  count: number;
  tone: "amber" | "red" | "violet";
  children: ReactNode;
}) {
  return (
    <section className="space-y-1.5">
      <div className="flex items-center gap-2 px-0.5">
        <span className="text-[10px] font-semibold uppercase tracking-wide text-zinc-500">
          {title}
        </span>
        <CountPill value={count} tone={tone} />
      </div>
      <div className="space-y-1.5">{children}</div>
    </section>
  );
}
