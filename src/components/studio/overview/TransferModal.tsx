import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { apiPost } from "@/lib/api";
import type { EffectiveStudioItem } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { cn } from "@/lib/utils";
import { inputClass, labelClass } from "./studio-overview-classes";

export function TransferModal({
  transfer,
  workspaces,
  workspaceId,
  onClose,
  onDone,
}: {
  transfer: { item: EffectiveStudioItem; mode: "copy" | "move" } | null;
  workspaces: Array<{ id: string; name: string }>;
  workspaceId: string | null;
  onClose: () => void;
  onDone: (message: string) => Promise<void>;
}) {
  const [mode, setMode] = useState<"copy" | "move">("copy");
  const [scope, setScope] = useState<"workspace" | "global">("workspace");
  const [targetWorkspace, setTargetWorkspace] = useState(workspaceId ?? "");
  const [newId, setNewId] = useState("");
  const [overwrite, setOverwrite] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!transfer) return;
    setMode(transfer.mode);
    setScope(
      transfer.item.source_scope === "workspace" ? "global" : "workspace",
    );
    setTargetWorkspace(workspaceId ?? "");
    setNewId(transfer.item.id);
    setOverwrite(false);
    setError(null);
  }, [transfer, workspaceId]);

  if (!transfer) return null;

  async function apply() {
    if (!transfer) return;
    setBusy(true);
    setError(null);
    try {
      await apiPost(
        `/api/studio/${transfer.item.kind}/${encodeURIComponent(transfer.item.id)}/${mode}?scope=${transfer.item.source_scope}${
          transfer.item.source_scope === "workspace" ? `&workspace_id=${workspaceId ?? ""}` : ""
        }`,
        {
          target_scope: scope,
          target_workspace_id: scope === "workspace" ? targetWorkspace : null,
          new_id: newId.trim() || transfer.item.id,
          overwrite,
        },
      );
      await onDone(
        mode === "move" ? "Item movido." : "Item copiado.",
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open onClose={onClose} title={`Copiar/mover ${transfer.item.id}`} size="md">
      <div className="space-y-3">
        <div className="flex gap-2">
          {(["copy", "move"] as const).map((value) => (
            <button
              key={value}
              type="button"
              onClick={() => setMode(value)}
              className={cn(
                "rounded-lg border px-3 py-1.5 text-[13px]",
                mode === value
                  ? "border-blue-500/50 bg-blue-500/10 text-blue-200"
                  : "border-zinc-800 text-zinc-500",
              )}
            >
              {value === "copy" ? "Copiar" : "Mover"}
            </button>
          ))}
        </div>
        <label className="block">
          <span className={labelClass}>Escopo destino</span>
          <select
            value={scope}
            onChange={(event) => setScope(event.target.value as "workspace" | "global")}
            className={inputClass}
          >
            <option value="workspace">workspace</option>
            <option value="global">global</option>
          </select>
        </label>
        {scope === "workspace" ? (
          <label className="block">
            <span className={labelClass}>Workspace destino</span>
            <select
              value={targetWorkspace}
              onChange={(event) => setTargetWorkspace(event.target.value)}
              className={inputClass}
            >
              {workspaces.map((workspace) => (
                <option key={workspace.id} value={workspace.id}>
                  {workspace.name}
                </option>
              ))}
            </select>
          </label>
        ) : null}
        <label className="block">
          <span className={labelClass}>Novo ID</span>
          <input
            value={newId}
            onChange={(event) => setNewId(event.target.value)}
            className={inputClass}
          />
        </label>
        <label className="flex items-center gap-2 text-[13px] text-zinc-400">
          <input
            type="checkbox"
            checked={overwrite}
            onChange={(event) => setOverwrite(event.target.checked)}
            className="accent-blue-500"
          />
          sobrescrever se já existir
        </label>
        {error ? (
          <p className="rounded border border-red-500/20 bg-red-500/10 p-2 text-[12px] text-red-300">
            {error}
          </p>
        ) : null}
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button size="sm" onClick={() => void apply()} disabled={busy}>
            {busy ? <Loader2 size={13} className="animate-spin" /> : null}
            {mode === "copy" ? "Copiar" : "Mover"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
