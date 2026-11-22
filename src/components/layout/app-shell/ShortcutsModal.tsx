import { Modal } from "@/components/ui/modal";

const SHORTCUTS: Array<[string, string]> = [
  ["Ctrl/Cmd + K", "Abrir busca global / command palette"],
  ["Ctrl/Cmd + N", "Criar nova tarefa"],
  ["Ctrl/Cmd + Shift + Space", "Quick Capture (texto, link ou print)"],
  ["Ctrl/Cmd + Shift + R", "Atualizar dados do workspace"],
  ["?", "Mostrar ou esconder esta ajuda"],
  ["Esc", "Fechar modal, drawer ou paleta"],
  ["Enter", "Abrir tarefa selecionada no Kanban"],
];

export function ShortcutsModal({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Atalhos do OpenDesk"
      description="Produtividade sem sair do teclado."
      size="md"
    >
      <div className="space-y-1.5">
        {SHORTCUTS.map(([keys, label]) => (
          <div
            key={keys}
            className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
          >
            <span className="text-[13.5px] text-zinc-300">{label}</span>
            <kbd className="rounded border border-zinc-700 bg-zinc-900 px-1.5 py-0.5 font-mono text-[12px] text-zinc-400">
              {keys}
            </kbd>
          </div>
        ))}
      </div>
    </Modal>
  );
}
