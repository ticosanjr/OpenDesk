import { useState } from "react";
import { Loader2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import {
  importJsonBundle,
  pickJsonBundle,
  type BundleEnvelope,
  type BundleKind,
  type ImportMode,
} from "@/lib/import-export";
import { toast } from "@/store/toast-store";

export function JsonImportButton({
  kind,
  endpoint,
  workspaceId,
  label = "Importar JSON",
  disabled = false,
  onImported,
}: {
  kind: BundleKind;
  endpoint: string;
  workspaceId: string | null;
  label?: string;
  disabled?: boolean;
  onImported?: () => void | Promise<void>;
}) {
  const [pending, setPending] = useState<{
    bundle: BundleEnvelope;
    fileName: string;
  } | null>(null);
  const [mode, setMode] = useState<ImportMode>("merge");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function choose() {
    setError(null);
    try {
      const picked = await pickJsonBundle(kind);
      if (picked) {
        setMode("merge");
        setPending(picked);
      }
    } catch (cause) {
      toast({
        tone: "error",
        title: "Bundle incompatível",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function apply() {
    if (!pending) return;
    setBusy(true);
    setError(null);
    try {
      await importJsonBundle(endpoint, pending.bundle, workspaceId, mode);
      toast({
        tone: "success",
        title: "Importação concluída",
        description: pending.fileName,
      });
      setPending(null);
      await onImported?.();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        onClick={() => void choose()}
        disabled={disabled}
      >
        <Upload size={13} />
        {label}
      </Button>
      <Modal
        open={pending !== null}
        onClose={() => {
          if (!busy) setPending(null);
        }}
        title="Confirmar importação"
        description={pending?.fileName}
        size="sm"
      >
        <div className="space-y-3">
          <label className="block">
            <span className="mb-1 block text-[12px] text-zinc-500">
              Como tratar itens existentes?
            </span>
            <select
              value={mode}
              onChange={(event) => setMode(event.target.value as ImportMode)}
              className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2.5 text-[13px] text-zinc-200 outline-none focus:border-blue-500/60"
              aria-label="Modo de importação"
            >
              <option value="merge">Mesclar (atualiza conflitos)</option>
              <option value="replace">Substituir</option>
            </select>
          </label>
          {mode === "replace" ? (
            <p className="rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-2 text-[12px] text-red-200">
              O modo Substituir descarta os itens atuais das seções incluídas
              neste bundle. A ação só começa depois desta confirmação.
            </p>
          ) : (
            <p className="text-[12px] text-zinc-500">
              Itens com a mesma identidade são atualizados; os demais são
              preservados.
            </p>
          )}
          {error ? (
            <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
              {error}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setPending(null)}
              disabled={busy}
            >
              Cancelar
            </Button>
            <Button
              size="sm"
              variant={mode === "replace" ? "destructive" : "default"}
              onClick={() => void apply()}
              disabled={busy}
            >
              {busy ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Upload size={13} />
              )}
              {mode === "replace" ? "Substituir" : "Mesclar"}
            </Button>
          </div>
        </div>
      </Modal>
    </>
  );
}
