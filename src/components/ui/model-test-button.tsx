import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { apiPost } from "@/lib/api";
import { cn } from "@/lib/utils";

interface ModelTestResponse {
  ok: boolean;
  latency_ms: number;
  variant?: string | null;
  response?: string | null;
  error_code?: string | null;
  error_message?: string | null;
}

function compactMessage(result: ModelTestResponse): string {
  if (result.ok) {
    return `OK · ${result.latency_ms}ms`;
  }
  return result.error_code ?? "erro";
}

function fullMessage(result: ModelTestResponse): string {
  if (result.ok) {
    const text = (result.response ?? "")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, 100);
    const variant = result.variant ? ` · variante ${result.variant}` : "";
    return `OK · ${result.latency_ms}ms${variant}${text ? ` · ${text}` : ""}`;
  }
  return `${result.error_code ?? "erro"}: ${
    (result.error_message ?? "falha ao testar o modelo").slice(0, 220)
  }`;
}

/**
 * Calls POST /api/models/test, which performs one stateless generation
 * (`Output only: OK`). No session, history or tools are created.
 */
export function ModelTestButton({
  model,
  workspaceId,
  variant = "full",
  label = "Testar (mínimo)",
  className,
}: {
  model: string | null | undefined;
  workspaceId?: string | null;
  variant?: "full" | "compact";
  label?: string;
  className?: string;
}) {
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<ModelTestResponse | null>(null);

  useEffect(() => {
    setResult(null);
  }, [model, workspaceId]);

  async function run() {
    if (!model) return;
    setTesting(true);
    setResult(null);
    try {
      const response = await apiPost<ModelTestResponse>("/api/models/test", {
        model,
        workspace_id: workspaceId ?? undefined,
      });
      setResult(response);
    } catch (cause) {
      setResult({
        ok: false,
        latency_ms: 0,
        error_code: "request_failed",
        error_message: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setTesting(false);
    }
  }

  if (!model) return null;

  const message = result
    ? variant === "compact"
      ? compactMessage(result)
      : fullMessage(result)
    : null;

  return (
    <div className={cn("space-y-1", className)}>
      <button
        type="button"
        onClick={() => void run()}
        disabled={testing}
        title="Faz uma geração curta sem sessão, histórico ou ferramentas. Consome poucos tokens."
        className={cn(
          "inline-flex items-center gap-1 rounded-md border border-zinc-800 text-zinc-400 transition-colors hover:border-zinc-700 hover:text-zinc-200 disabled:opacity-50",
          variant === "compact" ? "h-5 px-1.5 text-[11.5px]" : "h-6 px-2 text-[11.5px]",
        )}
      >
        {testing ? <Loader2 size={10} className="animate-spin" /> : null}
        {testing ? "Testando…" : label}
      </button>
      {message ? (
        <p
          className={cn(
            "break-words leading-snug",
            variant === "compact" ? "text-[11.5px]" : "text-[11.5px]",
            result?.ok ? "text-emerald-300" : "text-red-300",
          )}
          title={result ? fullMessage(result) : undefined}
        >
          {message}
        </p>
      ) : null}
    </div>
  );
}
