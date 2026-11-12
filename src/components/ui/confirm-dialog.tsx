import { useConfirmStore } from "@/store/confirm-store";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { TriangleAlert } from "lucide-react";

export function ConfirmDialog() {
  const options = useConfirmStore((state) => state.options);
  const settle = useConfirmStore((state) => state.settle);

  if (!options) return null;
  const destructive = options.tone === "destructive";

  return (
    <Modal
      open
      onClose={() => settle(false)}
      title={options.title}
      description={options.description}
      className="max-w-md"
      size="sm"
    >
      <div className="flex items-start gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5 text-[12px] text-zinc-400">
        <TriangleAlert
          size={16}
          className={destructive ? "mt-0.5 shrink-0 text-red-400" : "mt-0.5 shrink-0 text-amber-400"}
        />
        {destructive
          ? "Esta ação não pode ser desfeita pelo OpenDesk."
          : "Confirme para continuar."}
      </div>
      <div className="mt-4 flex justify-end gap-2 border-t border-zinc-800/70 pt-4">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => settle(false)}
          autoFocus={destructive}
        >
          {options.cancelLabel ?? "Cancelar"}
        </Button>
        <Button
          size="sm"
          variant={destructive ? "destructive" : "default"}
          onClick={() => settle(true)}
          autoFocus={!destructive}
        >
          {options.confirmLabel ?? "Confirmar"}
        </Button>
      </div>
    </Modal>
  );
}
