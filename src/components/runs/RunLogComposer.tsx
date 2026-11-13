import { Loader2, Send } from "lucide-react";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { Button } from "@/components/ui/button";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { SlashTextarea } from "@/components/ui/slash-textarea";

export function RunLogComposer({
  workspaceId,
  lastAgent,
  lastModel,
  composer,
  agent,
  model,
  continueExecution,
  sending,
  onComposerChange,
  onAgentChange,
  onModelChange,
  onContinueChange,
  onSend,
}: {
  workspaceId: string;
  lastAgent?: string | null;
  lastModel?: string | null;
  composer: string;
  agent: string;
  model: string | null;
  continueExecution: boolean;
  sending: boolean;
  onComposerChange: (value: string) => void;
  onAgentChange: (value: string) => void;
  onModelChange: (value: string | null) => void;
  onContinueChange: (value: boolean) => void;
  onSend: () => void;
}) {
  return (
    <div className="shrink-0 border-t border-zinc-800/70 bg-zinc-950/80 p-3">
      <p className="mb-2 text-[11.5px] text-zinc-500">
        Último agente/modelo:{" "}
        <span className="font-mono text-zinc-400">
          @{lastAgent ?? "router"} · {lastModel ?? "modelo"}
        </span>
      </p>
      <div className="mb-2 grid gap-2 sm:grid-cols-2">
        <AgentCombobox
          value={agent}
          onChange={onAgentChange}
          workspaceId={workspaceId}
          placeholder={`automático (router)${lastAgent ? ` · @${lastAgent}` : ""}`}
        />
        <ModelCombobox
          value={model}
          onChange={onModelChange}
          placeholder="modelo automático"
          routerHint={`Modelo automático. Último usado: ${lastModel ?? "desconhecido"}.`}
        />
      </div>
      <div className="flex w-full min-w-0 items-stretch gap-2">
        <SlashTextarea
          containerClassName="min-w-0 flex-1"
          value={composer}
          onChange={onComposerChange}
          workspaceId={workspaceId}
          rows={3}
          submitOnEnter
          onSubmit={onSend}
          placeholder="Responder, comentar ou pedir uma continuação… (Enter envia)"
          className="min-h-[88px] flex-1"
        />
        <Button
          size="sm"
          className="h-auto w-20 shrink-0 flex-col gap-1"
          onClick={onSend}
          disabled={sending || !composer.trim()}
          title="Enviar mensagem e continuar a mesma execução"
        >
          {sending ? <Loader2 size={15} className="animate-spin" /> : <Send size={15} />}
          Enviar
        </Button>
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-2">
        <label className="flex items-center gap-2 text-[13px] text-zinc-400">
          <input
            type="checkbox"
            checked={continueExecution}
            onChange={(event) => onContinueChange(event.target.checked)}
            className="accent-blue-500"
          />
          continuar na mesma execução
        </label>
        <span className="text-[11.5px] text-zinc-500">
          A continuação vira uma tentativa interna; o histórico continua no mesmo
          run.
        </span>
      </div>
    </div>
  );
}
