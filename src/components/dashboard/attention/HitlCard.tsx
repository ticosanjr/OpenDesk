import { Clock3, ExternalLink } from "lucide-react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import {
  hitlConsequences,
  hitlTypeLabel,
  parseHitlOptions,
} from "@/lib/hitl";
import { links } from "@/lib/links";
import type { HitlRequest } from "./types";

export function HitlCard({
  request,
  busy,
  feedbackOpen,
  feedbackText,
  onFeedbackText,
  onToggleFeedback,
  onApprove,
  onReject,
  onChoose,
  onFeedback,
}: {
  request: HitlRequest;
  busy: boolean;
  feedbackOpen: boolean;
  feedbackText: string;
  onFeedbackText: (value: string) => void;
  onToggleFeedback: () => void;
  onApprove: () => void;
  onReject: () => void;
  onChoose: (index: number, value: string) => void;
  onFeedback: () => void;
}) {
  const consequences = hitlConsequences(request);
  const options = parseHitlOptions(request.options_json);
  const isChoice = request.interaction_type === "MULTI_CHOICE";
  const isAck = request.interaction_type === "ACKNOWLEDGE";
  const supportsFeedback =
    request.interaction_type === "APPROVAL_WITH_FEEDBACK" ||
    request.interaction_type === "ARTIFACT_REVIEW";
  const expiry = request.expires_at
    ? new Date(request.expires_at).toLocaleString("pt-BR")
    : null;
  return (
    <div className="space-y-2 rounded-lg border border-amber-500/25 bg-zinc-950/50 p-3">
      <div className="flex flex-wrap items-start gap-2">
        <Clock3 size={14} className="mt-0.5 shrink-0 text-amber-300" />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <p className="text-[12px] font-medium text-amber-100">
              {request.title || "Receita aguardando decisão"}
            </p>
            <span className="rounded border border-amber-500/25 bg-amber-500/10 px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-wide text-amber-300">
              {hitlTypeLabel(request.interaction_type)}
            </span>
            <span className="font-mono text-[9px] text-zinc-500">
              {request.checkpoint_id}
            </span>
          </div>
          <p className="mt-0.5 text-[9px] text-zinc-500">
            {expiry ? `expira em ${expiry}` : "sem prazo informado"}
          </p>
        </div>
        <Link
          to={links.run(request.recipe_run_id)}
          className="inline-flex items-center gap-1 text-[10px] text-zinc-500 hover:text-zinc-300"
        >
          <ExternalLink size={10} />
          ver run
        </Link>
      </div>

      <p className="whitespace-pre-line rounded-lg border border-zinc-800/70 bg-zinc-900/40 px-3 py-2 text-[11px] leading-relaxed text-zinc-200">
        {request.question_markdown?.trim() ||
          "Este checkpoint precisa da sua decisão para o fluxo continuar."}
      </p>

      <div className="grid gap-1.5 sm:grid-cols-3">
        <div className="rounded-md border border-emerald-500/20 bg-emerald-500/[0.06] px-2 py-1.5">
          <p className="text-[9px] font-medium uppercase tracking-wide text-emerald-300">
            Se aprovar
          </p>
          <p className="mt-0.5 text-[9px] leading-snug text-zinc-400">
            {consequences.approve}
          </p>
        </div>
        <div className="rounded-md border border-red-500/20 bg-red-500/[0.06] px-2 py-1.5">
          <p className="text-[9px] font-medium uppercase tracking-wide text-red-300">
            Se rejeitar
          </p>
          <p className="mt-0.5 text-[9px] leading-snug text-zinc-400">
            {consequences.reject}
          </p>
        </div>
        <div className="rounded-md border border-amber-500/20 bg-amber-500/[0.06] px-2 py-1.5">
          <p className="text-[9px] font-medium uppercase tracking-wide text-amber-300">
            Se expirar
          </p>
          <p className="mt-0.5 text-[9px] leading-snug text-zinc-400">
            {consequences.timeout}
          </p>
        </div>
      </div>

      {isChoice && options.length > 0 ? (
        <div className="flex flex-wrap gap-1.5">
          {options.map((option, index) => (
            <Button
              key={`${request.token}-${index}`}
              size="sm"
              variant="outline"
              className="h-7"
              disabled={busy}
              onClick={() => onChoose(index, option)}
            >
              {index + 1}. {option}
            </Button>
          ))}
        </div>
      ) : null}

      {feedbackOpen ? (
        <div className="space-y-1.5">
          <textarea
            value={feedbackText}
            onChange={(event) => onFeedbackText(event.target.value)}
            rows={3}
            placeholder="Explique o que deve ser ajustado antes de refazer esta etapa."
            className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-950/60 px-2.5 py-2 text-[11px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-amber-500/50"
            aria-label="Ajustes solicitados"
          />
          <div className="flex justify-end gap-1.5">
            <Button size="sm" variant="ghost" className="h-7" onClick={onToggleFeedback}>
              Cancelar
            </Button>
            <Button
              size="sm"
              className="h-7"
              disabled={busy || !feedbackText.trim()}
              onClick={onFeedback}
            >
              Enviar ajustes
            </Button>
          </div>
        </div>
      ) : null}

      <div className="flex flex-wrap items-center justify-end gap-1.5">
        {!isChoice ? (
          <Button size="sm" className="h-7" disabled={busy} onClick={onApprove}>
            {isAck ? "Entendi, continuar" : "Aprovar e continuar"}
          </Button>
        ) : null}
        {supportsFeedback && !feedbackOpen ? (
          <Button
            size="sm"
            variant="outline"
            className="h-7"
            disabled={busy}
            onClick={onToggleFeedback}
          >
            Pedir ajustes
          </Button>
        ) : null}
        {!isAck ? (
          <Button
            size="sm"
            variant="outline"
            className="h-7 border-red-500/30 text-red-300"
            disabled={busy}
            onClick={onReject}
          >
            Rejeitar
          </Button>
        ) : null}
      </div>
    </div>
  );
}
