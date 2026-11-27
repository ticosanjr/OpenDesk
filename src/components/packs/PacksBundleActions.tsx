import { useState } from "react";
import { Download, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { JsonImportButton } from "@/components/ui/json-import-button";
import { apiGet } from "@/lib/api";
import { exportJsonBundle, type BundleEnvelope } from "@/lib/import-export";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";

export function PacksBundleActions({ onImported }: { onImported?: () => void }) {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [busy, setBusy] = useState(false);

  async function exportAll() {
    setBusy(true);
    try {
      const query = workspaceId
        ? `?workspace_id=${encodeURIComponent(workspaceId)}`
        : "";
      const bundle = await apiGet<BundleEnvelope>(
        `/api/bundles/packs/export${query}`,
      );
      const savedName = await exportJsonBundle(
        "packs_bundle",
        bundle.items,
        `opendesk-packs-${new Date().toISOString().slice(0, 10)}.json`,
        workspaceId,
      );
      if (!savedName) return;
      toast({ tone: "success", title: "Pacotes exportados" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível exportar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <Button
        variant="outline"
        size="sm"
        onClick={() => void exportAll()}
        disabled={busy}
      >
        {busy ? (
          <Loader2 size={13} className="animate-spin" />
        ) : (
          <Download size={13} />
        )}
        Exportar pacotes
      </Button>
      <JsonImportButton
        kind="packs_bundle"
        endpoint="/api/bundles/packs/import"
        workspaceId={workspaceId}
        label="Importar pacotes"
        disabled={!workspaceId}
        onImported={onImported}
      />
    </div>
  );
}
