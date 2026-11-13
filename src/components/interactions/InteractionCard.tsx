import { useMemo, useState } from "react";
import { CheckCircle2, MessageCircleQuestion, ShieldAlert } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { apiPost } from "@/lib/api";
import { toast } from "@/store/toast-store";
import type { TaskInteraction } from "@/lib/types";
import { cn } from "@/lib/utils";

function parseOptions(value: string): string[] {
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : [];
  } catch {
    return [];
  }
}

export function InteractionCard({
  interaction,
  onResolved,
}: {
  interaction: TaskInteraction;
  onResolved?: () => void;
}) {
  const options = useMemo(
    () => parseOptions(interaction.options_json),
    [interaction.options_json],
  );
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);

  const expires = interaction.expires_at
    ? new Date(interaction.expires_at).toLocaleString("pt-BR")
    : null;

  function toggle(option: string) {
    setSelected((current) => {
      if (!interaction.multiple) return new Set([option]);
      const next = new Set(current);
      if (next.has(option)) next.delete(option);
      else next.add(option);
      return next;
    });
  }

  async function submit() {
    if (selected.size === 0 && !text.trim()) return;
    setSending(true);
    try {
      await apiPost(`/api/interactions/${interaction.id}/resolve`, {
        selected: Array.from(selected),
        text: text.trim() || null,
      });
      toast({
        tone: "success",
        title: "Resposta enviada",
        description: "A execução foi retomada.",
      });
      onResolved?.();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível responder",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="space-y-3 rounded-xl border border-amber-500/30 bg-amber-500/[0.04] p-3">
      <div className="flex flex-wrap items-start gap-2">
        <MessageCircleQuestion size={15} className="mt-0.5 shrink-0 text-amber-300" />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <p className="text-[13.5px] font-medium text-amber-100">
              O agente precisa da sua resposta
            </p>
            <Badge variant="yellow">
              {interaction.multiple ? "múltipla escolha" : "escolha"}
            </Badge>
          </div>
          {expires ? (
            <p className="mt-0.5 text-[11.5px] text-zinc-500">expira em {expires}</p>
          ) : null}
        </div>
      </div>

      <p className="whitespace-pre-line rounded-lg border border-zinc-800/70 bg-zinc-900/40 px-3 py-2 text-[13px] leading-relaxed text-zinc-200">
        {interaction.question}
      </p>
      {interaction.context_markdown?.trim() ? (
        <pre className="max-h-40 overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800/70 bg-zinc-950/50 p-2 font-mono text-[12px] leading-relaxed text-zinc-400">
          {interaction.context_markdown}
        </pre>
      ) : null}

      {options.length > 0 ? (
        <div className="flex flex-wrap gap-1.5">
          {options.map((option) => {
            const active = selected.has(option);
            return (
              <button
                key={option}
                type="button"
                aria-pressed={active}
                onClick={() => toggle(option)}
                className={cn(
                  "rounded-lg border px-2.5 py-1.5 text-left text-[13px] transition-colors",
                  active
                    ? "border-amber-500/50 bg-amber-500/15 text-amber-100"
                    : "border-zinc-800 bg-zinc-950/40 text-zinc-400 hover:border-zinc-700 hover:text-zinc-200",
                )}
              >
                {option}
              </button>
            );
          })}
        </div>
      ) : null}

      {interaction.allow_custom ? (
        <textarea
          value={text}
          onChange={(event) => setText(event.target.value)}
          rows={3}
          placeholder="Ou escreva sua própria resposta…"
          className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-950/60 px-2.5 py-2 text-[13px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-amber-500/50"
          aria-label="Resposta livre"
        />
      ) : null}

      <div className="flex flex-wrap items-center justify-end gap-2">
        <Button
          size="sm"
          onClick={() => void submit()}
          disabled={sending || (selected.size === 0 && !text.trim())}
        >
          <CheckCircle2 size={12} />
          {sending ? "Enviando…" : "Enviar resposta"}
        </Button>
      </div>
    </div>
  );
}

export function PermissionRequestCard({
  request,
  onDecided,
}: {
  request: {
    request_id: string;
    permission_action: string;
    resources?: string | null;
    task_id?: string | null;
    run_id?: string | null;
  };
  onDecided?: () => void;
}) {
  const [sending, setSending] = useState<string | null>(null);

  async function decide(decision: "once" | "always" | "reject") {
    setSending(decision);
    try {
      await apiPost(`/api/permissions/${request.request_id}/reply`, { decision });
      toast({
        tone: decision === "reject" ? "warning" : "success",
        title:
          decision === "reject"
            ? "Permissão negada"
            : decision === "always"
              ? "Permissão liberada nesta tarefa"
              : "Permissão liberada uma vez",
      });
      onDecided?.();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível responder à permissão",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSending(null);
    }
  }

  return (
    <div className="space-y-2 rounded-xl border border-amber-500/30 bg-amber-500/[0.04] p-3">
      <div className="flex items-start gap-2">
        <ShieldAlert size={15} className="mt-0.5 shrink-0 text-amber-300" />
        <div>
          <p className="text-[13.5px] font-medium text-amber-100">
            O agente pediu permissão
          </p>
          <p className="mt-0.5 font-mono text-[12px] text-amber-200/80">
            {request.permission_action}
            {request.resources ? ` · ${request.resources}` : ""}
          </p>
          <p className="mt-1 text-[11.5px] text-zinc-500">
            Permitir uma vez vale só para esta ação; “nesta tarefa” cobre ações
            iguais até o fim do run; negar bloqueia o run.
          </p>
        </div>
      </div>
      <div className="flex flex-wrap justify-end gap-1.5">
        <Button
          size="sm"
          className="h-7"
          disabled={sending !== null}
          onClick={() => void decide("once")}
        >
          Permitir uma vez
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="h-7"
          disabled={sending !== null}
          onClick={() => void decide("always")}
        >
          Nesta tarefa
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="h-7 border-red-500/30 text-red-300"
          disabled={sending !== null}
          onClick={() => void decide("reject")}
        >
          Negar
        </Button>
      </div>
    </div>
  );
}
