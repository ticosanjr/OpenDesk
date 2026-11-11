import type { ReactNode } from "react";

export function SectionTitle({
  icon,
  title,
  hint,
}: {
  icon: ReactNode;
  title: string;
  hint?: string;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-zinc-400">{icon}</span>
      <h2 className="text-[12px] font-medium text-zinc-200">{title}</h2>
      {hint ? <span className="text-[9px] text-zinc-500">{hint}</span> : null}
    </div>
  );
}
