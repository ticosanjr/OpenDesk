import { FileInput, FolderOpen } from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { formatNumber } from "@/lib/format";
import type { TimelineInput } from "@/lib/types";

function parentFolder(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index > 0 ? path.slice(0, index) : path;
}

export function RunInputsStrip({ inputs }: { inputs: TimelineInput[] }) {
  if (inputs.length === 0) return null;
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-zinc-800/70 bg-zinc-950/40 px-4 py-2">
      <span className="mr-1 inline-flex items-center gap-1 font-mono text-[11.5px] uppercase tracking-wide text-zinc-500">
        <FileInput size={11} />
        arquivos de entrada
      </span>
      {inputs.slice(0, 8).map((input) => (
        <span
          key={input.path}
          className="inline-flex max-w-full items-center gap-1 rounded-md border border-zinc-700/70 bg-zinc-900/50 px-1.5 py-0.5 text-[11.5px] text-zinc-300"
          title={input.absolute_path}
        >
          <span className="max-w-48 truncate">{input.name}</span>
          {input.size != null ? (
            <span className="font-mono text-[10.5px] text-zinc-500">
              {formatNumber(input.size)} B
            </span>
          ) : null}
          <Button
            variant="ghost"
            size="icon"
            className="h-5 w-5"
            aria-label={`Abrir arquivo de entrada ${input.name}`}
            title="Abrir arquivo"
            onClick={() => void openPath(input.absolute_path).catch(() => {})}
          >
            <FileInput size={10} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-5 w-5"
            aria-label={`Abrir pasta do arquivo ${input.name}`}
            title="Abrir pasta"
            onClick={() => void openPath(parentFolder(input.absolute_path)).catch(() => {})}
          >
            <FolderOpen size={10} />
          </Button>
          {input.trigger_type ? (
            <Badge variant="default" className="px-1 py-0 text-[10px]">
              {input.trigger_type}
            </Badge>
          ) : null}
        </span>
      ))}
      {inputs.length > 8 ? (
        <span className="text-[11.5px] text-zinc-500">
          +{inputs.length - 8}
        </span>
      ) : null}
    </div>
  );
}
