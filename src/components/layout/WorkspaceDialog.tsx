import { useEffect, useMemo, useState } from "react";
import { FolderOpen, FolderPlus, Link2, Loader2 } from "lucide-react";
import { Modal } from "@/components/ui/modal";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useUiStore } from "@/store/ui-store";
import { useAppStore } from "@/store/app-store";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";
import { joinPath, pickFolder, slugifyWorkspace } from "@/lib/folder";
import { cn } from "@/lib/utils";

type Mode = "create" | "existing";

function basename(path: string): string {
  const normalized = path.replace(/[\\/]+$/, "");
  const parts = normalized.split(/[\\/]/);
  return parts[parts.length - 1] || "";
}

export function WorkspaceDialog() {
  const open = useUiStore((state) => state.workspaceDialogOpen);
  const setOpen = useUiStore((state) => state.setWorkspaceDialogOpen);
  const createWorkspace = useAppStore((state) => state.createWorkspace);
  const refresh = useAppStore((state) => state.refresh);

  const [mode, setMode] = useState<Mode>("create");
  const [name, setName] = useState("");
  const [rootParent, setRootParent] = useState("");
  const [existingPath, setExistingPath] = useState("");
  const [color, setColor] = useState("#3b82f6");
  const [policy, setPolicy] = useState("COST_BENEFIT");
  const [blueprints, setBlueprints] = useState<
    Array<{ id: string; name: string; description: string }>
  >([]);
  const [blueprint, setBlueprint] = useState("");
  const [useAsDefault, setUseAsDefault] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    void apiGet<Array<{ id: string; name: string; description: string }>>(
      "/api/blueprints",
    )
      .then(setBlueprints)
      .catch(() => setBlueprints([]));
    apiGet<{ workspaces_root: string }>("/api/app/defaults")
      .then((defaults) => {
        setRootParent((current) => current || defaults.workspaces_root);
      })
      .catch((cause) => {
        setError(cause instanceof Error ? cause.message : String(cause));
      });
  }, [open]);

  const createTarget = useMemo(() => {
    if (!name.trim()) return "";
    return joinPath(rootParent, slugifyWorkspace(name));
  }, [name, rootParent]);

  const targetPath = mode === "create" ? createTarget : existingPath;

  async function chooseParent() {
    const selected = await pickFolder(
      "Pasta onde os workspaces serão criados",
      rootParent || undefined,
    );
    if (selected) setRootParent(selected);
  }

  async function chooseExisting() {
    const selected = await pickFolder(
      "Selecione a pasta do workspace existente",
      existingPath || rootParent || undefined,
    );
    if (selected) {
      setExistingPath(selected);
      if (!name.trim()) setName(basename(selected));
    }
  }

  async function submit() {
    if (!name.trim()) {
      setError("Informe o nome do workspace.");
      return;
    }
    if (!targetPath.trim()) {
      setError(
        mode === "create"
          ? "Escolha a pasta pai ou informe o caminho."
          : "Selecione a pasta do workspace existente.",
      );
      return;
    }

    setSaving(true);
    setError(null);
    try {
      const workspace = await createWorkspace({
        name: name.trim(),
        root_path: targetPath.trim(),
        color_hex: color,
        default_router_policy: policy,
      });

      if (blueprint) {
        try {
          const applied = await apiPost<{ created: string[]; skipped: string[] }>(
            `/api/blueprints/${blueprint}/apply?workspace_id=${encodeURIComponent(workspace.id)}`,
            {},
          );
          toast({
            tone: "success",
            title: "Equipe aplicada",
            description: `${applied.created.length} arquivos criados, ${applied.skipped.length} preservados.`,
          });
        } catch (cause) {
          toast({
            tone: "warning",
            title: "Workspace criado, equipe não aplicada",
            description: cause instanceof Error ? cause.message : String(cause),
          });
        }
      }

      if (mode === "create" && useAsDefault && rootParent.trim()) {
        await apiPut("/api/settings/workspaces_root", rootParent.trim());
      }

      await refresh();
      setName("");
      setExistingPath("");
      setUseAsDefault(false);
      setBlueprint("");
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
      title="Workspace"
      description="Crie uma pasta nova em uma raiz escolhida ou registre uma pasta que já existe. O OpenDesk só adiciona input/, output/ e .opencode/ quando faltarem."
      className="max-w-xl"
    >
      <div className="space-y-4">
        <div className="flex items-center gap-1 rounded-lg border border-zinc-800/80 bg-zinc-900/30 p-1">
          <button
            type="button"
            onClick={() => {
              setMode("create");
              setError(null);
            }}
            className={cn(
              "flex flex-1 items-center justify-center gap-1.5 rounded-md px-2.5 py-1.5 text-[11px] font-medium transition-colors",
              mode === "create"
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-200",
            )}
          >
            <FolderPlus size={13} />
            Criar novo
          </button>
          <button
            type="button"
            onClick={() => {
              setMode("existing");
              setError(null);
            }}
            className={cn(
              "flex flex-1 items-center justify-center gap-1.5 rounded-md px-2.5 py-1.5 text-[11px] font-medium transition-colors",
              mode === "existing"
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-200",
            )}
          >
            <Link2 size={13} />
            Adicionar existente
          </button>
        </div>

        <label className="block">
          <span className="mb-1.5 block text-[11px] font-medium text-zinc-400">
            Nome
          </span>
          <input
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="Neuralab, Bernardes AI, Home Lab…"
            className={inputClass}
          />
        </label>

        {mode === "create" ? (
          <>
            <label className="block">
              <span className="mb-1.5 block text-[11px] font-medium text-zinc-400">
                Pasta onde os workspaces serão criados
              </span>
              <div className="flex gap-2">
                <div className="relative min-w-0 flex-1">
                  <FolderOpen
                    size={15}
                    className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-600"
                  />
                  <input
                    value={rootParent}
                    onChange={(event) => setRootParent(event.target.value)}
                    placeholder="~/Documents/OpenDesk Workareas"
                    className={cn(inputClass, "pl-9 font-mono text-[11px]")}
                  />
                </div>
                <Button variant="outline" size="sm" className="h-9 shrink-0" onClick={() => void chooseParent()}>
                  Escolher
                </Button>
              </div>
            </label>

            <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
              <p className="text-[9px] uppercase tracking-[0.12em] text-zinc-600">
                pasta do workspace
              </p>
              <p className="mt-0.5 truncate font-mono text-[10px] text-zinc-300">
                {createTarget || "preencha o nome do workspace"}
              </p>
            </div>

            <label className="flex items-center gap-2 text-[11px] text-zinc-400">
              <input
                type="checkbox"
                checked={useAsDefault}
                onChange={(event) => setUseAsDefault(event.target.checked)}
                className="accent-blue-500"
              />
              Usar esta pasta como padrão para os próximos workspaces
            </label>
          </>
        ) : (
          <label className="block">
            <span className="mb-1.5 block text-[11px] font-medium text-zinc-400">
              Pasta do workspace existente
            </span>
            <div className="flex gap-2">
              <div className="relative min-w-0 flex-1">
                <FolderOpen
                  size={15}
                  className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-600"
                />
                <input
                  value={existingPath}
                  onChange={(event) => setExistingPath(event.target.value)}
                  placeholder="/home/rafael/Projetos/minha-empresa"
                  className={cn(inputClass, "pl-9 font-mono text-[11px]")}
                />
              </div>
              <Button variant="outline" size="sm" className="h-9 shrink-0" onClick={() => void chooseExisting()}>
                Escolher
              </Button>
            </div>
            <p className="mt-1 text-[9px] leading-relaxed text-zinc-600">
              Nada é movido. O OpenDesk registra o caminho e cria input/, output/ e
              .opencode/ somente se não existirem.
            </p>
          </label>
        )}

        <div className="grid grid-cols-[1fr_110px] gap-3">
          <label className="block">
            <span className="mb-1.5 block text-[11px] font-medium text-zinc-400">
              Política padrão
            </span>
            <select
              value={policy}
              onChange={(event) => setPolicy(event.target.value)}
              className={cn(inputClass, "px-2.5 text-[12px]")}
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

        <label className="block">
          <span className="mb-1.5 block text-[11px] font-medium text-zinc-400">
            Equipe inicial (opcional)
          </span>
          <select
            value={blueprint}
            onChange={(event) => setBlueprint(event.target.value)}
            className={cn(inputClass, "px-2.5 text-[12px]")}
          >
            <option value="">Nenhuma — começar vazio</option>
            {blueprints.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
          <p className="mt-1 text-[9px] leading-relaxed text-zinc-600">
            Cria agentes e skills em .opencode/ sem sobrescrever arquivos existentes.
          </p>
        </label>

        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
            {error}
          </p>
        ) : null}

        <div className="flex items-center justify-between gap-3 border-t border-zinc-800/70 pt-4">
          <div className="flex min-w-0 items-center gap-2">
            <Badge variant="default">SSOT</Badge>
            <p className="truncate text-[9px] text-zinc-600">
              Configuração continua em opencode.jsonc e .opencode/.
            </p>
          </div>
          <div className="flex shrink-0 gap-2">
            <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
              Cancelar
            </Button>
            <Button size="sm" onClick={() => void submit()} disabled={saving}>
              {saving ? <Loader2 size={14} className="animate-spin" /> : null}
              {mode === "create" ? "Criar workspace" : "Adicionar workspace"}
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60";
