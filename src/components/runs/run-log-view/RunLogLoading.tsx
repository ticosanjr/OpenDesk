import { Loader2 } from "lucide-react";

export function RunLogLoading() {
  return (
    <div className="flex items-center gap-2 p-6 text-[13px] text-zinc-500">
      <Loader2 size={14} className="animate-spin" />
      Carregando execução…
    </div>
  );
}
