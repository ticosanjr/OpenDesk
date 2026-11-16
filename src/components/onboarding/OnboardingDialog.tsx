import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  CheckCircle2,
  Circle,
  Loader2,
  RefreshCw,
  ShieldCheck,
} from "lucide-react";
import { Modal } from "@/components/ui/modal";
import { Button } from "@/components/ui/button";
import { apiGet } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { OnboardingStatus, OnboardingStep } from "@/lib/types/onboarding";

export function OnboardingDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setStatus(await apiGet<OnboardingStatus>("/api/onboarding"));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    void load();
  }, [load, open]);

  function openStep(step: OnboardingStep) {
    onClose();
    navigate(step.href);
  }

  const required = status?.steps.filter((step) => step.required) ?? [];
  const optional = status?.steps.filter((step) => !step.required) ?? [];

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Primeiros passos"
      description="Configure o essencial para o OpenDesk operar. Tudo pode ser refeito depois em Configurações."
      size="md"
      initialFocus="dialog"
    >
      <div className="space-y-4">
        {loading && !status ? (
          <div className="flex items-center gap-2 py-6 text-[13px] text-zinc-400">
            <Loader2 size={15} className="animate-spin" />
            Verificando integrações…
          </div>
        ) : null}
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12.5px] text-red-300">
            {error}
          </p>
        ) : null}

        {status ? (
          <>
            <div className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
              <div className="flex items-center gap-2 text-[13px]">
                <ShieldCheck
                  size={15}
                  className={status.complete ? "text-emerald-300" : "text-amber-300"}
                />
                <span className="text-zinc-300">
                  Essenciais: {status.required_done}/{status.required_total}
                  {status.optional_total > 0
                    ? ` · opcionais: ${status.optional_done}/${status.optional_total}`
                    : ""}
                </span>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void load()}
                disabled={loading}
                className="h-7 gap-1.5"
              >
                <RefreshCw size={12} className={loading ? "animate-spin" : undefined} />
                Verificar
              </Button>
            </div>

            {status.complete ? (
              <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[12.5px] text-emerald-300">
                Tudo pronto. O workspace está configurado para operar.
              </p>
            ) : null}

            <StepGroup title="Essenciais" steps={required} onOpen={openStep} />
            {optional.length > 0 ? (
              <StepGroup title="Opcionais" steps={optional} onOpen={openStep} />
            ) : null}
          </>
        ) : null}
      </div>
    </Modal>
  );
}

function StepGroup({
  title,
  steps,
  onOpen,
}: {
  title: string;
  steps: OnboardingStep[];
  onOpen: (step: OnboardingStep) => void;
}) {
  return (
    <section className="space-y-1.5">
      <h3 className="px-1 font-mono text-[11px] uppercase tracking-[0.14em] text-zinc-500">
        {title}
      </h3>
      {steps.map((step) => (
        <button
          key={step.id}
          type="button"
          onClick={() => onOpen(step)}
          className="flex w-full items-start gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2 text-left transition-colors hover:border-zinc-700 hover:bg-zinc-900/50"
        >
          {step.status === "done" ? (
            <CheckCircle2 size={16} className="mt-0.5 shrink-0 text-emerald-300" />
          ) : (
            <Circle
              size={16}
              className={cn(
                "mt-0.5 shrink-0",
                step.required ? "text-amber-300" : "text-zinc-500",
              )}
            />
          )}
          <span className="min-w-0 flex-1">
            <span className="flex items-center gap-2">
              <span className="truncate text-[13.5px] font-medium text-zinc-200">
                {step.label}
              </span>
              {!step.required ? (
                <span className="rounded border border-zinc-700 px-1 font-mono text-[10px] uppercase tracking-wide text-zinc-500">
                  opcional
                </span>
              ) : null}
            </span>
            <span className="mt-0.5 block text-[12px] leading-relaxed text-zinc-500">
              {step.detail}
            </span>
          </span>
        </button>
      ))}
    </section>
  );
}
