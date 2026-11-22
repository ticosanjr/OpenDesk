import type { ReactNode } from "react";

import { Card, CardContent } from "@/components/ui/card";

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
    <Card>
      <CardContent className="p-3">
        <div className="flex items-center gap-1.5 text-zinc-500">
          {icon}
          <span className="text-[11.5px] uppercase tracking-[0.12em]">{label}</span>
        </div>
        <p className="mt-1 font-mono text-lg text-zinc-100">{value}</p>
        <p className="text-[11.5px] text-zinc-500">{hint}</p>
      </CardContent>
    </Card>
  );
}
