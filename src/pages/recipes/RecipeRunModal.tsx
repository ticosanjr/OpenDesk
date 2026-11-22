import { Loader2, Play } from "lucide-react";
import { Modal } from "@/components/ui/modal";
import { Button } from "@/components/ui/button";
import { IconLabel } from "@/components/ui/icon-label";
import { PromptAttachments } from "@/components/ui/prompt-attachments";

export function RecipeRunModal({
  open,
  mode,
  context,
  workspaceId,
  attachments,
  running,
  onContextChange,
  onAttachmentsChange,
  onClose,
  onRun,
}: {
  open: boolean;
  mode: "dry" | "run" | null;
  context: string;
  workspaceId: string | null;
  attachments: string[];
  running: boolean;
  onContextChange: (value: string) => void;
  onAttachmentsChange: (value: string[]) => void;
  onClose: () => void;
  onRun: () => void;
}) {
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={mode === "dry" ? "Dry-run da receita" : "Executar receita"}
      description="O contexto abaixo fica disponível como {{trigger.context}}, {{trigger.prompt}} e {{trigger.question}} nos prompts."
      size="md"
    >
      <div className="space-y-3">
        <label className="block">
          <span className="mb-1.5 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
            Contexto / pergunta (opcional)
          </span>
          <textarea
            value={context}
            onChange={(event) => onContextChange(event.target.value)}
            rows={6}
            placeholder="Ex.: analise o contrato em juridico/contratos/atual.pdf"
            className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-900/50 p-2.5 font-mono text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
          />
          <PromptAttachments
            workspaceId={workspaceId}
            value={attachments}
            onChange={onAttachmentsChange}
            className="mt-2"
          />
        </label>
        <p className="text-[12px] leading-snug text-zinc-500">
          Para anexar arquivos, coloque-os na pasta de entrada do pacote e
          referencie o caminho no contexto. O dry-run não executa ferramentas
          externas.
        </p>
        <div className="flex justify-end gap-2 border-t border-zinc-800/70 pt-3">
          <Button
            variant="ghost"
            size="sm"
            onClick={onClose}
            disabled={running}
          >
            Cancelar
          </Button>
          <Button
            size="sm"
            onClick={onRun}
            disabled={running}
          >
            <IconLabel
              icon={
                running ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Play size={13} />
                )
              }
            >
              {mode === "dry" ? "Iniciar dry-run" : "Executar agora"}
            </IconLabel>
          </Button>
        </div>
      </div>
    </Modal>
  );
}
