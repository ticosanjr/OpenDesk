import { FolderOpen, Loader2, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import type { RestoreMode, WorkspaceBackup } from "@/lib/types";
import { cn } from "@/lib/utils";
import { fieldInputClass as inputClass, fieldLabelClass as labelClass } from "@/lib/ui-classes";

export function RestoreBackupModal({
  backup,
  mode,
  name,
  path,
  passphrase,
  error,
  restoring,
  onMode,
  onName,
  onPath,
  onPassphrase,
  onChooseTarget,
  onClose,
  onRestore,
}: {
  backup: WorkspaceBackup | null;
  mode: RestoreMode;
  name: string;
  path: string;
  passphrase: string;
  error: string | null;
  restoring: boolean;
  onMode: (mode: RestoreMode) => void;
  onName: (name: string) => void;
  onPath: (path: string) => void;
  onPassphrase: (value: string) => void;
  onChooseTarget: () => void;
  onClose: () => void;
  onRestore: () => void;
}) {
  return (
    <Modal
      open={backup !== null}
      onClose={onClose}
      title="Restaurar backup"
      description="Novo workspace mantém os arquivos atuais intactos; restaurar no lugar cria um backup de segurança antes de substituir."
      size="md"
    >
      <div className="space-y-3">
        <label>
          <span className={labelClass}>Modo</span>
          <select
            value={mode}
            onChange={(event) => onMode(event.target.value as RestoreMode)}
            className={inputClass}
          >
            <option value="new_workspace">Restaurar como nova workspace</option>
            <option value="in_place">
              Substituir workspace exatamente como estava
            </option>
          </select>
        </label>
        {mode === "new_workspace" ? (
          <>
            <label>
              <span className={labelClass}>Nome do novo workspace</span>
              <input
                value={name}
                onChange={(event) => onName(event.target.value)}
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Pasta de destino</span>
              <div className="flex gap-1.5">
                <input
                  value={path}
                  onChange={(event) => onPath(event.target.value)}
                  placeholder="/home/user/OpenDesk Workareas/workspace-restaurado"
                  className={cn(inputClass, "font-mono text-[12px]")}
                />
                <Button
                  variant="outline"
                  size="sm"
                  className="h-9"
                  onClick={onChooseTarget}
                >
                  <FolderOpen size={13} />
                </Button>
              </div>
            </label>
          </>
        ) : null}
        <label>
          <span className={labelClass}>Senha dos segredos (se o backup tiver)</span>
          <input
            type="password"
            value={passphrase}
            onChange={(event) => onPassphrase(event.target.value)}
            className={inputClass}
          />
        </label>
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            {error}
          </p>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button size="sm" disabled={restoring} onClick={onRestore}>
            {restoring ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <RotateCcw size={13} />
            )}
            Restaurar
          </Button>
        </div>
      </div>
    </Modal>
  );
}
