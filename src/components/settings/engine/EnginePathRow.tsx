import { Check, FileSearch, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";

export function EnginePathRow({
  manualPath,
  onManualPathChange,
  busy,
  onChoose,
  onApply,
}: {
  manualPath: string;
  onManualPathChange: (value: string) => void;
  busy: string | null;
  onChoose: () => void;
  onApply: () => void;
}) {
  return (
    <div className="flex flex-col gap-2 md:flex-row">
      <input
        value={manualPath}
        onChange={(event) => onManualPathChange(event.target.value)}
        placeholder="/home/usuario/.opencode/bin/opencode"
        className="h-9 min-w-0 flex-1 rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 font-mono text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
      />
      <div className="flex flex-wrap gap-2">
        <Button
          variant="outline"
          size="sm"
          className="h-9"
          onClick={() => onChoose()}
        >
          <FileSearch size={13} />
          Escolher
        </Button>
        <Button
          size="sm"
          className="h-9"
          onClick={() => onApply()}
          disabled={busy !== null}
        >
          {busy === "apply" ? (
            <Loader2 size={13} className="animate-spin" />
          ) : (
            <Check size={13} />
          )}
          Aplicar caminho
        </Button>
      </div>
    </div>
  );
}
