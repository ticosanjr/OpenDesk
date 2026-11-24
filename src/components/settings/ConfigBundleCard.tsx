import { useRef, useState } from "react";
import { Download, FileJson, Loader2, Upload } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { apiPost } from "@/lib/api";
import { CONFIG_BUNDLE_PRESETS, downloadConfigBundle } from "@/lib/config-bundle";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";

type Mode = "merge-skip-existing" | "merge-overwrite" | "replace-section";
type Preview = {
  sections?: Record<
    string,
    { incoming?: number; existing?: number; create?: number; update?: number; skip?: number }
  >;
};

const PRESETS: Array<{ id: string; label: string }> = [
  { id: "all", label: "Tudo (exceto segredos)" },
  { id: "memory", label: "Memórias & contexto" },
  { id: "prompts", label: "Prompts & Studio" },
  { id: "ops", label: "Roteador, fila & governança" },
];

export function ConfigBundleCard() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [preset, setPreset] = useState("all");
  const [mode, setMode] = useState<Mode>("merge-skip-existing");
  const [busy, setBusy] = useState<string | null>(null);
  const [bundle, setBundle] = useState<unknown>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [fileName, setFileName] = useState("");

  async function exportBundle() {
    if (!activeWorkspaceId) return;
    setBusy("export");
    try {
      const name = await downloadConfigBundle(
        activeWorkspaceId,
        CONFIG_BUNDLE_PRESETS[preset] ?? [],
      );
      toast({ tone: "success", title: "Bundle exportado", description: name });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível exportar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function readFile(file: File) {
    setBusy("read");
    setPreview(null);
    try {
      const parsed = JSON.parse(await file.text()) as unknown;
      setBundle(parsed);
      setFileName(file.name);
      const result = await apiPost<Preview>("/api/config-bundle/preview", {
        workspace_id: activeWorkspaceId,
        bundle: parsed,
        mode,
      });
      setPreview(result);
    } catch (cause) {
      setBundle(null);
      toast({
        tone: "error",
        title: "JSON inválido",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function applyImport() {
    if (!activeWorkspaceId || !bundle) return;
    setBusy("import");
    try {
      const report = await apiPost<Preview>("/api/config-bundle/import", {
        workspace_id: activeWorkspaceId,
        bundle,
        mode,
      });
      setPreview(report);
      toast({ tone: "success", title: "Bundle importado" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Importação falhou",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  return (
    <Card id="settings-config-bundle">
      <CardHeader className="flex flex-row items-center justify-between gap-2 py-3">
        <CardTitle className="flex items-center gap-2 text-[14px]">
          <FileJson size={14} className="text-zinc-400" />
          Exportar / importar configuração
        </CardTitle>
        <Badge variant="blue">sem segredos</Badge>
      </CardHeader>
      <CardContent className="space-y-3 p-4">
        <p className="text-[13px] leading-relaxed text-zinc-500">
          Um JSON único com memórias, contexto pessoal, prompts, aprendizado,
          roteador, budget, fila e rotas de automação. Senhas, tokens e chaves
          são removidos antes de gravar o arquivo.
        </p>
        <div className="flex flex-wrap items-end gap-2">
          <label className="min-w-[220px] flex-1">
            <span className="mb-1 block text-[12px] text-zinc-500">Preset</span>
            <select
              value={preset}
              onChange={(event) => setPreset(event.target.value)}
              className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2.5 text-[13px] text-zinc-200 outline-none"
            >
              {PRESETS.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.label}
                </option>
              ))}
            </select>
          </label>
          <Button
            variant="outline"
            size="sm"
            className="h-9"
            disabled={!activeWorkspaceId || busy === "export"}
            onClick={() => void exportBundle()}
          >
            {busy === "export" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Download size={13} />
            )}
            Exportar JSON
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-9"
            disabled={!activeWorkspaceId || busy === "read"}
            onClick={() => inputRef.current?.click()}
          >
            {busy === "read" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Upload size={13} />
            )}
            Escolher JSON
          </Button>
          <input
            ref={inputRef}
            type="file"
            accept="application/json,.json"
            className="hidden"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) void readFile(file);
              event.target.value = "";
            }}
          />
        </div>
        {fileName ? (
          <p className="font-mono text-[11.5px] text-zinc-500">{fileName}</p>
        ) : null}
        {preview?.sections ? (
          <div className="space-y-2">
            <div className="overflow-x-auto rounded-lg border border-zinc-800/70">
              <table className="w-full text-left text-[12px]">
                <thead className="bg-zinc-900/50 text-zinc-500">
                  <tr>
                    <th className="px-2 py-1.5 font-medium">seção</th>
                    <th className="px-2 py-1.5 font-medium">novos</th>
                    <th className="px-2 py-1.5 font-medium">atualizar</th>
                    <th className="px-2 py-1.5 font-medium">existentes</th>
                  </tr>
                </thead>
                <tbody>
                  {Object.entries(preview.sections).map(([name, value]) => (
                    <tr key={name} className="border-t border-zinc-800/60">
                      <td className="px-2 py-1.5 font-mono text-zinc-300">{name}</td>
                      <td className="px-2 py-1.5 text-emerald-300">{value.create ?? 0}</td>
                      <td className="px-2 py-1.5 text-amber-300">{value.update ?? 0}</td>
                      <td className="px-2 py-1.5 text-zinc-400">{value.existing ?? 0}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="flex flex-wrap items-end gap-2">
              <label className="min-w-[220px] flex-1">
                <span className="mb-1 block text-[12px] text-zinc-500">
                  Conflitos
                </span>
                <select
                  value={mode}
                  onChange={(event) => setMode(event.target.value as Mode)}
                  className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2.5 text-[13px] text-zinc-200 outline-none"
                >
                  <option value="merge-skip-existing">Manter existentes</option>
                  <option value="merge-overwrite">Sobrescrever conflitos</option>
                  <option value="replace-section">Substituir seções</option>
                </select>
              </label>
              <Button
                size="sm"
                className="h-9"
                disabled={busy === "import"}
                onClick={() => void applyImport()}
              >
                {busy === "import" ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Upload size={13} />
                )}
                Confirmar importação
              </Button>
            </div>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
