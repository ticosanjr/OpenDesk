import { Loader2, Play } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ScriptRunResult } from "@/lib/types";
import { cn } from "@/lib/utils";
import { fieldInputClass as inputClass } from "@/lib/ui-classes";

export function ScriptToolTestPanel({
  input,
  setInput,
  result,
  busy,
  disabled,
  onTest,
}: {
  input: string;
  setInput: (value: string) => void;
  result: ScriptRunResult | null;
  busy: boolean;
  disabled: boolean;
  onTest: () => void;
}) {
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-2.5">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-[12.5px] font-medium text-zinc-300">Testar script</span>
        <Button variant="outline" size="sm" disabled={disabled || busy} onClick={onTest}>
          {busy ? <Loader2 size={12} className="animate-spin" /> : <Play size={12} />}
          Testar
        </Button>
      </div>
      <textarea
        value={input}
        onChange={(event) => setInput(event.target.value)}
        rows={3}
        spellCheck={false}
        aria-label="JSON de entrada do teste"
        className={cn(inputClass, "mt-2 h-auto resize-y py-2 font-mono text-[12px]")}
      />
      {result ? (
        <div className="mt-2 space-y-1 font-mono text-[11.5px]">
          <div className="flex flex-wrap gap-1.5">
            <Badge variant={result.ok ? "green" : "red"}>
              {result.timed_out ? "timeout" : `exit ${result.exit_code ?? "?"}`}
            </Badge>
            <Badge variant="default">{result.duration_ms} ms</Badge>
          </div>
          {result.stdout ? (
            <pre className="max-h-36 overflow-auto whitespace-pre-wrap rounded bg-zinc-950 p-2 text-emerald-200">
              {result.stdout}
            </pre>
          ) : null}
          {result.stderr ? (
            <pre className="max-h-36 overflow-auto whitespace-pre-wrap rounded bg-zinc-950 p-2 text-red-300">
              {result.stderr}
            </pre>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
