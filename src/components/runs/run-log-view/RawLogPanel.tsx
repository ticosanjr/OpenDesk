import { Loader2 } from "lucide-react";

export function RawLogPanel({
  raw,
  rawLoading,
}: {
  raw: string | null;
  rawLoading: boolean;
}) {
  return (
    <div className="min-h-0 flex-1 overflow-auto p-4">
      {rawLoading ? (
        <p className="flex items-center gap-2 text-[12px] text-zinc-500">
          <Loader2 size={12} className="animate-spin" />
          Carregando log bruto…
        </p>
      ) : (
        <pre className="whitespace-pre-wrap break-words font-mono text-[11.5px] leading-relaxed text-zinc-400">
          {raw || "Log bruto indisponível."}
        </pre>
      )}
    </div>
  );
}
