import type { ReactNode } from "react";
import { FieldHint } from "@/components/ui/field-hint";

export function LabeledField({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <label className="block space-y-1">
      <span className="inline-flex items-center text-[12px] text-zinc-400">
        {label}
        {hint ? <FieldHint hint={hint} side="trailing" /> : null}
      </span>
      {children}
    </label>
  );
}

export const inputClass =
  "h-8 w-full rounded border border-zinc-800 bg-zinc-900/50 px-2 text-[12px] text-zinc-200 outline-none focus:border-blue-500/60";
