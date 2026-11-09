import { FolderPlus, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { IconLabel } from "@/components/ui/icon-label";
import { Modal } from "@/components/ui/modal";
import { PathInput } from "@/components/ui/path-input";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import type { WorkspaceTemplate } from "./pack-types";

export function WorkspaceTemplateModal({
  template,
  name,
  root,
  color,
  creating,
  errors,
  onName,
  onRoot,
  onColor,
  onCreate,
  onClose,
}: {
  template: WorkspaceTemplate | null;
  name: string;
  root: string;
  color: string;
  creating: boolean;
  errors: string[];
  onName: (value: string) => void;
  onRoot: (value: string) => void;
  onColor: (value: string) => void;
  onCreate: () => void;
  onClose: () => void;
}) {
  return (
    <Modal
      open={template !== null}
      onClose={onClose}
      title={template ? `Criar workspace: ${template.name}` : "Novo workspace"}
      description={template?.description}
      className="max-w-lg"
    >
      {template ? (
        <div className="space-y-3">
          <label className="block">
            <span className={labelClass}>Nome do workspace</span>
            <input
              value={name}
              onChange={(event) => onName(event.target.value)}
              className={inputClass}
            />
          </label>
          <label className="block">
            <span className={labelClass}>Pasta</span>
            <PathInput
              value={root}
              onChange={onRoot}
              title="Escolher pasta do novo workspace"
              placeholder="vazio usa OpenDesk Workareas/<nome>"
              hint="A pasta precisa estar fora de /tmp. O OpenDesk cria a estrutura .opencode nela."
            />
          </label>
          <label className="block">
            <span className={labelClass}>Cor</span>
            <input
              type="color"
              value={color}
              onChange={(event) => onColor(event.target.value)}
              className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-1"
            />
          </label>
          <p className="text-[10px] leading-snug text-zinc-500">
            Pacotes: {template.packs.join(", ")}. Rotinas entram inativas.
          </p>
          {errors.length > 0 ? (
            <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
              <p className="font-semibold">Instalação parcial</p>
              <ul className="mt-1 list-disc space-y-0.5 pl-4">
                {errors.map((error) => (
                  <li key={error}>{error}</li>
                ))}
              </ul>
            </div>
          ) : null}
          <div className="flex justify-end gap-2 border-t border-zinc-800/70 pt-3">
            <Button variant="ghost" size="sm" onClick={onClose}>
              Cancelar
            </Button>
            <Button size="sm" onClick={onCreate} disabled={creating}>
              <IconLabel
                icon={
                  creating ? (
                    <RefreshCw size={13} className="animate-spin" />
                  ) : (
                    <FolderPlus size={13} />
                  )
                }
              >
                Criar workspace
              </IconLabel>
            </Button>
          </div>
        </div>
      ) : null}
    </Modal>
  );
}
