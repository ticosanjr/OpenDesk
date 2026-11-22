import { Github, Globe } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { SOURCES, type SourceType } from "./install-sources";

export function InstallSourcePicker({
  lockSource,
  initialSource,
  sourceType,
  onSelect,
}: {
  lockSource: boolean;
  initialSource?: { sourceType: SourceType; value: string; kind?: string };
  sourceType: SourceType;
  onSelect: (sourceType: SourceType) => void;
}) {
  return lockSource && initialSource ? (
    <div className="flex items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
      {initialSource.sourceType === "url" ? (
        <Globe size={15} className="shrink-0 text-zinc-400" />
      ) : (
        <Github size={15} className="shrink-0 text-zinc-400" />
      )}
      <div className="min-w-0 flex-1">
        <p className="text-[13.5px] font-medium text-zinc-200">
          {initialSource.sourceType === "url"
            ? "URL direta"
            : "Repositório GitHub"}
        </p>
        <p className="truncate font-mono text-[13px] text-zinc-500">
          {initialSource.value}
        </p>
      </div>
      <Badge variant="blue">skill</Badge>
    </div>
  ) : (
    <div className="grid gap-1 sm:grid-cols-3">
      {SOURCES.map((source) => (
        <button
          key={source.id}
          type="button"
          onClick={() => onSelect(source.id)}
          className={cn(
            "flex items-start gap-2 rounded-lg border px-2.5 py-2 text-left",
            sourceType === source.id
              ? "border-blue-500/50 bg-blue-500/10"
              : "border-zinc-800 bg-zinc-950/40 hover:border-zinc-700",
          )}
        >
          <source.icon size={14} className="mt-0.5 text-zinc-400" />
          <div>
            <p className="text-[13.5px] font-medium text-zinc-200">{source.label}</p>
            <p className="mt-0.5 text-[13px] leading-relaxed text-zinc-500">
              {source.hint}
            </p>
          </div>
        </button>
      ))}
    </div>
  );
}
