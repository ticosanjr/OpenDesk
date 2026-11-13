import { FolderPlus, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { IconLabel } from "@/components/ui/icon-label";
import { Modal } from "@/components/ui/modal";
import { PathInput } from "@/components/ui/path-input";
import { Field, Input } from "@/components/ui/field";
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
      size="md"
    >
      {template ? (
        <div className="space-y-3">
          <Field label="Nome do workspace" htmlFor="template-name">
            <Input
              id="template-name"
              value={name}
              onChange={(event) => onName(event.target.value)}
            />
          </Field>
          <Field label="Pasta" htmlFor="template-root">
            <PathInput
              id="template-root"
              value={root}
              onChange={onRoot}
              title="Escolher pasta do novo workspace"
              placeholder="vazio usa OpenDesk Workareas/<nome>"
              hint="A pasta precisa estar fora de /tmp. O OpenDesk cria a estrutura .opencode nela."
            />
          </Field>
          <Field label="Cor" htmlFor="template-color">
            <input
              id="template-color"
              type="color"
              value={color}
              onChange={(event) => onColor(event.target.value)}
              className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-1"
            />
          </Field>
          <p className="text-[12px] leading-snug text-zinc-500">
            Pacotes: {template.packs.join(", ")}. Rotinas entram inativas.
          </p>
          <div className="flex flex-wrap gap-1">
            {template.packs.map((pack) => (
              <span
                key={pack}
                className="rounded border border-zinc-800 bg-zinc-900/60 px-1.5 py-0.5 font-mono text-[11px] text-zinc-500"
              >
                {pack}
              </span>
            ))}
          </div>
          <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-2">
            <p className="text-[11.5px] uppercase tracking-[0.12em] text-zinc-500">
              pastas iniciais
            </p>
            <p className="mt-1 font-mono text-[11.5px] leading-relaxed text-zinc-400">
              {template.folders.join(" · ")}
            </p>
          </div>
          {template.readme ? (
            <details className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-2">
              <summary className="cursor-pointer text-[12px] text-zinc-400">
                Ver detalhes do workspace
              </summary>
              <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap font-sans text-[12px] leading-relaxed text-zinc-400">
                {template.readme}
              </pre>
            </details>
          ) : null}
          {errors.length > 0 ? (
            <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-200">
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
