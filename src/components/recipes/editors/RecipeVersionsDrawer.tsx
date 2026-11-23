import { useCallback, useEffect, useState } from "react";
import { Loader2, RotateCcw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { apiGet, apiPost } from "@/lib/api";
import { relativeTime } from "@/lib/format";
import { toast } from "@/store/toast-store";

interface RecipeVersionSummary {
  id: string;
  content_sha256: string;
  size_bytes: number;
  created_at: string;
}

export function RecipeVersionsDrawer({
  recipeId,
  workspaceId,
  open,
  onClose,
  onRestored,
}: {
  recipeId: string | null;
  workspaceId: string;
  open: boolean;
  onClose: () => void;
  onRestored: () => void;
}) {
  const [versions, setVersions] = useState<RecipeVersionSummary[]>([]);
  const [diff, setDiff] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const query = workspaceId
    ? `?workspace_id=${encodeURIComponent(workspaceId)}`
    : "";

  const load = useCallback(async () => {
    if (!recipeId) return;
    setLoading(true);
    try {
      const value = await apiGet<RecipeVersionSummary[]>(
        `/api/recipes/${encodeURIComponent(recipeId)}/versions${query}`,
      );
      setVersions(Array.isArray(value) ? value : []);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível listar versões",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [recipeId, query]);

  useEffect(() => {
    if (open) {
      setDiff(null);
      void load();
    }
  }, [open, load]);

  async function showDiff(version: RecipeVersionSummary) {
    if (!recipeId) return;
    setBusy(version.id);
    try {
      const value = await apiGet<{ diff: string }>(
        `/api/recipes/${encodeURIComponent(recipeId)}/versions/${encodeURIComponent(version.id)}/diff${query}`,
      );
      setDiff(value.diff || "(sem diferenças)");
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível abrir o diff",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function restore(version: RecipeVersionSummary) {
    if (!recipeId) return;
    if (!window.confirm(`Restaurar a versão ${version.id.slice(0, 8)}?`)) return;
    setBusy(version.id);
    try {
      await apiPost(
        `/api/recipes/${encodeURIComponent(recipeId)}/rollback${query}`,
        { version_id: version.id },
      );
      toast({ tone: "success", title: "Versão restaurada" });
      onRestored();
      onClose();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível restaurar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Versões da receita"
      description="Cada save cria um snapshot imutável. O diff é contra o YAML atual."
      size="lg"
    >
      <div className="space-y-3">
        {loading ? (
          <p className="flex items-center gap-2 text-[13px] text-zinc-500">
            <Loader2 size={14} className="animate-spin" /> Carregando versões…
          </p>
        ) : versions.length === 0 ? (
          <p className="text-[13px] text-zinc-500">Nenhuma versão registrada ainda.</p>
        ) : (
          versions.map((version) => (
            <div
              key={version.id}
              className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-zinc-800/70 px-3 py-2"
            >
              <div>
                <p className="font-mono text-[12px] text-zinc-200">
                  {version.id.slice(0, 12)}…
                </p>
                <p className="text-[11.5px] text-zinc-500">
                  {relativeTime(version.created_at)} · {version.size_bytes} bytes
                </p>
              </div>
              <div className="flex items-center gap-1">
                <Badge variant="default">{version.content_sha256.slice(0, 8)}</Badge>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={busy === version.id}
                  onClick={() => void showDiff(version)}
                >
                  Diff
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={busy === version.id}
                  onClick={() => void restore(version)}
                >
                  <RotateCcw size={12} /> Restaurar
                </Button>
              </div>
            </div>
          ))
        )}
        {diff ? (
          <pre className="max-h-72 overflow-auto rounded-lg border border-zinc-800 bg-zinc-950/60 p-3 font-mono text-[11.5px] text-zinc-300">
            {diff}
          </pre>
        ) : null}
      </div>
    </Modal>
  );
}
