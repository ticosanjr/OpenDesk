import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { apiGet, apiPost } from "@/lib/api";
import type { BackupEntry, EffectiveStudioItem } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { confirmAction } from "@/store/confirm-store";

export function BackupsModal({
  item,
  workspaceId,
  onClose,
  onRestored,
}: {
  item: EffectiveStudioItem | null;
  workspaceId: string | null;
  onClose: () => void;
  onRestored: () => Promise<void>;
}) {
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (!item) return;
    setLoading(true);
    setError(null);
    const query = `scope=${item.source_scope}${
      item.source_scope === "workspace" && workspaceId
        ? `&workspace_id=${workspaceId}`
        : ""
    }`;
    apiGet<{ backups: BackupEntry[] }>(
      `/api/studio/${item.kind}/${encodeURIComponent(item.id)}/backups?${query}`,
    )
      .then((value) => setBackups(value.backups))
      .catch((cause) =>
        setError(cause instanceof Error ? cause.message : String(cause)),
      )
      .finally(() => setLoading(false));
  }, [item, workspaceId]);

  if (!item) return null;

  async function restore(backup: BackupEntry) {
    if (!item) return;
    if (!(await confirmAction({ title: "Restaurar backup?", description: `O conteúdo atual será substituído pelo backup de ${backup.created_at}.`, confirmLabel: "Restaurar", tone: "destructive" }))) return;
    setBusy(backup.id);
    setError(null);
    try {
      const query = `scope=${item.source_scope}${
        item.source_scope === "workspace" && workspaceId
          ? `&workspace_id=${workspaceId}`
          : ""
      }`;
      await apiPost(
        `/api/studio/${item.kind}/${encodeURIComponent(item.id)}/restore?${query}`,
        { backup_id: backup.id },
      );
      await onRestored();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Modal
      open
      onClose={onClose}
      title={`Backups de ${item.id}`}
      description="Cada gravação cria um backup antes de sobrescrever o arquivo nativo."
      size="md"
    >
      <div className="space-y-2">
        {error ? (
          <p className="rounded border border-red-500/20 bg-red-500/10 p-2 text-[12px] text-red-300">
            {error}
          </p>
        ) : null}
        {loading ? (
          <p className="flex items-center gap-2 text-[12px] text-zinc-500">
            <Loader2 size={12} className="animate-spin" />
            carregando…
          </p>
        ) : backups.length === 0 ? (
          <p className="text-[12px] text-zinc-500">
            Nenhum backup para este item ainda.
          </p>
        ) : (
          backups.map((backup) => (
            <div
              key={backup.id}
              className="flex items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
            >
              <div className="min-w-0 flex-1">
                <p className="text-[12px] text-zinc-300">{backup.created_at}</p>
                <p className="truncate font-mono text-[11.5px] text-zinc-500">
                  {backup.id}
                </p>
              </div>
              <Button
                variant="outline"
                size="sm"
                className="h-7"
                onClick={() => void restore(backup)}
                disabled={busy !== null}
              >
                {busy === backup.id ? (
                  <Loader2 size={12} className="animate-spin" />
                ) : null}
                restaurar
              </Button>
            </div>
          ))
        )}
      </div>
    </Modal>
  );
}
