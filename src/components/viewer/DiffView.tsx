import { useMemo } from "react";
import { Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import { parseUnifiedDiff } from "@/lib/diff";
import { cn } from "@/lib/utils";

export function DiffView({ patch }: { patch: string }) {
  const parsed = useMemo(() => parseUnifiedDiff(patch), [patch]);

  if (!patch.trim()) {
    return (
      <p className="p-4 text-[13px] text-zinc-500">
        Sem diff registrado para este artefato.
      </p>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-zinc-800/70 px-3 py-1.5">
        <span className="font-mono text-[11.5px] uppercase tracking-wide text-zinc-500">
          diff unificado
        </span>
        <span className="font-mono text-[12px] text-emerald-300">
          +{parsed.additions}
        </span>
        <span className="font-mono text-[12px] text-red-300">
          -{parsed.deletions}
        </span>
        <Button
          variant="ghost"
          size="sm"
          className="ml-auto h-6 px-1.5 text-[11.5px]"
          onClick={() => void navigator.clipboard.writeText(patch)}
          title="Copiar o diff"
        >
          <Copy size={10} />
          copiar diff
        </Button>
      </div>
      <div className="scroll-area min-h-0 flex-1 overflow-auto">
        <table className="w-full min-w-[620px] border-collapse font-mono text-[12px] leading-relaxed">
          <tbody>
            {parsed.lines.map((line, index) =>
              line.kind === "hunk" ? (
                <tr key={index} className="bg-blue-500/[0.06] text-blue-200">
                  <td colSpan={3} className="px-3 py-1">
                    {line.text}
                  </td>
                </tr>
              ) : line.kind === "meta" ? (
                <tr key={index} className="text-zinc-500">
                  <td colSpan={3} className="px-3 py-0.5">
                    {line.text}
                  </td>
                </tr>
              ) : (
                <tr
                  key={index}
                  className={cn(
                    line.kind === "add"
                      ? "bg-emerald-500/10 text-emerald-200"
                      : line.kind === "del"
                        ? "bg-red-500/10 text-red-200"
                        : "text-zinc-400",
                  )}
                >
                  <td className="w-10 select-none border-r border-zinc-800/60 px-2 py-0 text-right text-[11.5px] text-zinc-600">
                    {line.oldLine ?? ""}
                  </td>
                  <td className="w-10 select-none border-r border-zinc-800/60 px-2 py-0 text-right text-[11.5px] text-zinc-600">
                    {line.newLine ?? ""}
                  </td>
                  <td className="whitespace-pre px-3 py-0">
                    {line.kind === "add" ? "+" : line.kind === "del" ? "-" : " "}
                    {line.text}
                  </td>
                </tr>
              ),
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
