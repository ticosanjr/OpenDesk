import { useState } from "react";
import { Loader2, Wand2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { apiPost } from "@/lib/api";

export function MetaBuilder({ workspaceId }: { workspaceId: string }) {
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    kind: string;
    id: string;
    path: string;
    message: string;
  } | null>(null);

  async function build() {
    if (!prompt.trim()) {
      setError("Descreva o agente, skill ou comando.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const value = await apiPost<typeof result>(
        `/api/meta/build?workspace_id=${encodeURIComponent(workspaceId)}&scope=workspace`,
        { prompt: prompt.trim() },
      );
      setResult(value);
      setPrompt("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-3">
      <div className="flex w-full items-stretch gap-2">
        <SlashTextarea
          containerClassName="min-w-0 flex-1"
          value={prompt}
          onChange={setPrompt}
          workspaceId={workspaceId}
          rows={2}
          placeholder="Crie um subagente chamado revisor-fiscal focado em contabilidade…"
          className="min-h-[52px] flex-1"
        />
        <Button
          size="sm"
          className="h-auto min-h-[52px] w-28 shrink-0"
          onClick={() => void build()}
          disabled={busy}
        >
          {busy ? <Loader2 size={14} className="animate-spin" /> : <Wand2 size={14} />}
          Criar arquivo
        </Button>
      </div>
      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}
      {result ? (
        <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/[0.06] p-3 text-[13px] text-emerald-200">
          <p className="font-medium">{result.message}</p>
          <p className="mt-1 font-mono text-[11.5px] text-emerald-300/70">
            {result.kind} · {result.id} · {result.path}
          </p>
        </div>
      ) : null}
    </div>
  );
}
