import { AlertTriangle, RefreshCw, Settings, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { formatUsd } from "@/lib/format";

interface BudgetAlert {
  level: string;
  message: string;
  daily_spent: number;
  daily_limit: number;
}

interface AppBannersProps {
  loading: boolean;
  error: string | null;
  engineOffline: boolean;
  daemonMessage?: string | null;
  budgetAlert: BudgetAlert | null;
  dismissedBudget: string | null;
  retryingEngine: boolean;
  onRetryBoot: () => void;
  onConfigureEngine: () => void;
  onAdjustBudget: () => void;
  onRestartDaemon: () => void;
  onDismissBudget: (message: string) => void;
}

export function AppBanners({
  loading,
  error,
  engineOffline,
  daemonMessage,
  budgetAlert,
  dismissedBudget,
  retryingEngine,
  onRetryBoot,
  onConfigureEngine,
  onAdjustBudget,
  onRestartDaemon,
  onDismissBudget,
}: AppBannersProps) {
  return (
    <>
      {loading ? (
        <div className="h-0.5 w-full overflow-hidden bg-zinc-900">
          <div className="h-full w-1/3 animate-[loading_1.2s_ease-in-out_infinite] bg-blue-500/70" />
        </div>
      ) : null}
      {error ? (
        <div className="flex items-center gap-3 border-b border-red-500/20 bg-red-500/10 px-4 py-2 text-[13px] text-red-300">
          <AlertTriangle size={14} />
          <span className="flex-1">{error}</span>
          <Button
            variant="destructive"
            size="sm"
            onClick={onRetryBoot}
            className="h-7"
            disabled={retryingEngine}
          >
            <RefreshCw
              size={12}
              className={retryingEngine ? "animate-spin" : undefined}
            />
            Tentar novamente
          </Button>
        </div>
      ) : null}
      {engineOffline ? (
        <div className="flex flex-wrap items-center gap-3 border-b border-amber-500/20 bg-amber-500/10 px-4 py-2 text-[13px] text-amber-200">
          <AlertTriangle size={14} className="shrink-0" />
          <span className="min-w-0 flex-1">
            <strong className="font-semibold">OpenCode offline.</strong>{" "}
            {daemonMessage ??
              "Nenhum binário foi detectado e nenhum serviço compartilhado respondeu."}{" "}
            Nada será enfileirado até o motor voltar.
          </span>
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={onConfigureEngine}
          >
            <Settings size={12} />
            Configurar binário
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={onRestartDaemon}
            disabled={retryingEngine}
          >
            <RefreshCw
              size={12}
              className={retryingEngine ? "animate-spin" : undefined}
            />
            Tentar novamente
          </Button>
        </div>
      ) : null}
      {budgetAlert && dismissedBudget !== budgetAlert.message ? (
        <div
          className={
            budgetAlert.level === "hard"
              ? "flex flex-wrap items-center gap-3 border-b border-red-500/25 bg-red-500/10 px-4 py-2 text-[13px] text-red-200"
              : "flex flex-wrap items-center gap-3 border-b border-amber-500/25 bg-amber-500/10 px-4 py-2 text-[13px] text-amber-200"
          }
        >
          <AlertTriangle size={14} className="shrink-0" />
          <span className="min-w-0 flex-1">
            <strong className="font-semibold">
              {budgetAlert.level === "hard"
                ? "Orçamento bloqueado."
                : "Orçamento próximo do limite."}{" "}
            </strong>
            {budgetAlert.message} ({formatUsd(budgetAlert.daily_spent)} de{" "}
            {formatUsd(budgetAlert.daily_limit)})
          </span>
          <Button variant="outline" size="sm" className="h-7" onClick={onAdjustBudget}>
            Ajustar limite
          </Button>
          <button
            type="button"
            aria-label="Fechar aviso de orçamento"
            className="rounded p-1 opacity-60 hover:opacity-100"
            onClick={() => onDismissBudget(budgetAlert.message)}
          >
            <X size={13} />
          </button>
        </div>
      ) : null}
    </>
  );
}
