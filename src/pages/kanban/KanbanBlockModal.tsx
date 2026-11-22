import type { Dispatch, SetStateAction } from "react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";

export function KanbanBlockModal({
  open,
  onClose,
  blockReason,
  setBlockReason,
  onConfirm,
}: {
  open: boolean;
  onClose: () => void;
  blockReason: string;
  setBlockReason: Dispatch<SetStateAction<string>>;
  onConfirm: () => void;
}) {
  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Mover para Atenção"
      description="Explique por que esta tarefa precisa de intervenção humana."
      size="md"
    >
      <div className="space-y-3">
        <label className="block">
          <span className="mb-1.5 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
            Motivo do bloqueio
          </span>
          <textarea
            value={blockReason}
            onChange={(event) => setBlockReason(event.target.value)}
            rows={3}
            autoFocus
            placeholder="Ex.: falta acesso ao Drive, precisa revisar o escopo..."
            className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 py-2 text-[13px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
          />
        </label>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button size="sm" onClick={onConfirm}>
            Bloquear tarefa
          </Button>
        </div>
      </div>
    </Modal>
  );
}
