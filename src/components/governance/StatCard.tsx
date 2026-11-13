import type { ReactNode } from "react";

export function StatCard({
  icon,
  label,
  value,
  hint,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  hint: string;
}) {
  return (
    <div className="bg-zinc-950/25 p-3.5">
      <div className="flex items-center gap-1.5 text-zinc-400">
        {icon}
        <span className="text-[13px] font-medium uppercase tracking-[0.08em]">
          {label}
        </span>
      </div>
      <p className="mt-1.5 font-mono text-[24px] leading-tight tracking-[-0.02em] text-zinc-100 [font-variant-numeric:tabular-nums]">
        {value}
      </p>
      <p className="mt-0.5 text-[13.5px] text-zinc-400">{hint}</p>
    </div>
  );
}
