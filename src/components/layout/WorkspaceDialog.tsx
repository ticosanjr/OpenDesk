import { useState } from "react";
import { FolderOpen, Loader2 } from "lucide-react";
import { Modal } from "@/components/ui/modal";
import { Button } from "@/components/ui/button";
import { useUiStore } from "@/store/ui-store";
import { useAppStore } from "@/store/app-store";
import { ROUTER_POLICY_LABELS } from "@/lib/types";

export function WorkspaceDialog() {
  const open = useUiStore((state) => state.workspaceDialogOpen);
  const setOpen = useUiStore((state) => state.setWorkspaceDialogOpen);
  const createWorkspace = useAppStore((state) => state.createWorkspace);
  const refresh = useAppStore((state) => state.refresh);

  const [name, setName] = useState("");
  const [rootPath, setRootPath] = useState("");
  const [color, setColor] = useState("#3b82f6");
  const [policy, setPolicy] = useState("COST_BENEFIT");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    if (!name.trim() || !rootPath.trim()) {
      setError("Informe o nome e o caminho do workspace.");
      return;
    }

    setSaving(true);
    setError(null);
    try {
      await createWorkspace({
        name: name.trim(),
        root_path: rootPath.trim(),
        color_hex: color,
        default_router_policy: policy,
      });
      await refresh();
      setName("");
      setRootPath("");
      setOpen(false);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      open={open}
      onClose={() => setOpen(false)}
      title="Novo workspace"
      description="O OpenDesk cria input/ e output/ se não existirem. O restante do diretório não é alterado."
    >
      <div className="space-y-4">
        <label className="block">
          <span className="mb-1.5 block text-[11px] font-medium text-zinc-400">
            Nome
          </span>
          <input
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="Neuralab, Bernardes AI, Home Lab…"
            className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 text-sm text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60"
          />
        </label>

        <label className="block">
          <span className="mb-1.5 block text-[11px] font-medium text-zinc-400">
            Caminho no disco
          </span>
          <div className="relative">
            <FolderOpen
              size={15}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-600"
            />
            <input
              value={rootPath}
              onChange={(event) => setRootPath(event.target.value)}
              placeholder="/home/rafael/Projetos/meu-workspace"
              className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 pl-9 pr-3 font-mono text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60"
            />
          </div>
        </label>

        <div className="grid grid-cols-[1fr_110px] gap-3">
          <label className="block">
            <span className="mb-1.5 block text-[11px] font-medium text-zinc-400">
              Política padrão
            </span>
            <select
              value={policy}
              onChange={(event) => setPolicy(event.target.value)}
              className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12px] text-zinc-200 outline-none focus:border-blue-500/60"
            >
              {Object.entries(ROUTER_POLICY_LABELS).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>

          <label className="block">
            <span className="mb-1.5 block text-[11px] font-medium text-zinc-400">
              Cor
            </span>
            <div className="flex h-9 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2">
              <input
                type="color"
                value={color}
                onChange={(event) => setColor(event.target.value)}
                className="h-5 w-7 cursor-pointer rounded border-0 bg-transparent p-0"
              />
              <span className="font-mono text-[10px] text-zinc-500">{color}</span>
            </div>
          </label>
        </div>

        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
            {error}
          </p>
        ) : null}

        <div className="flex items-center justify-between border-t border-zinc-800/70 pt-4">
          <p className="max-w-[60%] text-[10px] leading-relaxed text-zinc-600">
            O workspace é registrado no SQLite local; a configuração do OpenCode
            continua em <span className="font-mono">opencode.jsonc</span> e{" "}
            <span className="font-mono">.opencode/</span>.
          </p>
          <div className="flex gap-2">
            <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
              Cancelar
            </Button>
            <Button size="sm" onClick={() => void submit()} disabled={saving}>
              {saving ? <Loader2 size={14} className="animate-spin" /> : null}
              Criar workspace
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}
