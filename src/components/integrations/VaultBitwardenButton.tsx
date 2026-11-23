import { useState } from "react";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { apiPost } from "@/lib/api";
import { toast } from "@/store/toast-store";

export function VaultBitwardenButton({
  workspaceId,
  onImported,
}: {
  workspaceId: string;
  onImported: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [session, setSession] = useState("");
  const [itemIds, setItemIds] = useState("");
  const [busy, setBusy] = useState(false);

  async function importItems() {
    if (!session.trim()) return;
    setBusy(true);
    try {
      const ids = itemIds
        .split(/[\s,]+/)
        .map((value) => value.trim())
        .filter(Boolean);
      const value = await apiPost<{ imported: unknown[] }>(
        "/api/vault/import-bitwarden",
        {
          session,
          workspace_id: workspaceId,
          item_ids: ids.length ? ids : undefined,
        },
      );
      toast({
        tone: "success",
        title: `${value.imported.length} item(ns) importado(s) do Bitwarden`,
      });
      setSession("");
      setItemIds("");
      setOpen(false);
      onImported();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao importar do Bitwarden",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <Button variant="ghost" size="sm" onClick={() => setOpen(true)}>
        Bitwarden
      </Button>
      <Modal
        open={open}
        onClose={() => {
          setSession("");
          setItemIds("");
          setOpen(false);
        }}
        title="Importar do Bitwarden"
        description="Cole a sessão desbloqueada (BW_SESSION). Ela é usada apenas no processo do bw e não é salva."
        size="md"
      >
        <div className="space-y-3">
          <input
            type="password"
            value={session}
            onChange={(event) => setSession(event.target.value)}
            placeholder="BW_SESSION"
            autoComplete="off"
            className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 font-mono text-[12px] text-zinc-100 outline-none focus:border-blue-500/60"
          />
          <input
            value={itemIds}
            onChange={(event) => setItemIds(event.target.value)}
            placeholder="IDs de itens (opcional; vazio = todos)"
            className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 font-mono text-[12px] text-zinc-100 outline-none focus:border-blue-500/60"
          />
          <p className="text-[11.5px] text-zinc-500">
            Rode <code className="font-mono">bw unlock</code> e copie a sessão devolvida. Os
            itens importados começam com HITL e sem ACL.
          </p>
          <div className="flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
              Cancelar
            </Button>
            <Button size="sm" disabled={busy || !session.trim()} onClick={() => void importItems()}>
              {busy ? <Loader2 size={13} className="animate-spin" /> : null} Importar
            </Button>
          </div>
        </div>
      </Modal>
    </>
  );
}
