import { useCallback, useEffect, useState } from "react";
import { Download, Loader2, Save, ShieldCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PathInput } from "@/components/ui/path-input";
import { Switch } from "@/components/ui/switch";
import {
  loadFilePreferences,
  saveFilePreferences,
  type FilePreferences,
} from "@/lib/file-preferences";
import { toast } from "@/store/toast-store";

export function FilePreferencesCard() {
  const [preferences, setPreferences] = useState<FilePreferences>({
    downloadDir: "",
    downloadAsk: true,
    importExportAsk: true,
    htmlAllowScripts: false,
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setPreferences(await loadFilePreferences());
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function savePreferences() {
    setSaving(true);
    try {
      await saveFilePreferences(preferences);
      toast({ tone: "success", title: "Preferências de arquivos salvas" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Download size={15} className="text-zinc-500" />
            Downloads e visualizador
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Escolha onde os arquivos baixados são salvos e se o visualizador
            executa scripts HTML por padrão.
          </p>
        </div>
        <Badge variant="default">local</Badge>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="space-y-1.5">
          <span className="block text-[12px] font-medium uppercase tracking-[0.1em] text-zinc-400">
            Pasta padrão de downloads
          </span>
          <PathInput
            value={preferences.downloadDir}
            onChange={(value) =>
              setPreferences((current) => ({ ...current, downloadDir: value }))
            }
            title="Escolher pasta de downloads"
            placeholder="vazio pergunta a cada download"
            hint="Quando o diálogo está desligado, os arquivos vão direto para esta pasta."
            disabled={loading}
          />
        </div>
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
          <div>
            <p className="text-[12.5px] font-medium text-zinc-300">
              Perguntar sempre onde salvar ao importar/exportar
            </p>
            <p className="text-[11.5px] text-zinc-500">
              Quando desligado, exportações vão direto para a pasta padrão.
            </p>
          </div>
          <Switch
            checked={preferences.importExportAsk}
            onCheckedChange={(value) =>
              setPreferences((current) => ({
                ...current,
                importExportAsk: value,
              }))
            }
            label="Perguntar ao importar/exportar"
            disabled={loading}
          />
        </div>
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
          <div>
            <p className="text-[12.5px] font-medium text-zinc-300">
              Perguntar onde salvar
            </p>
            <p className="text-[11.5px] text-zinc-500">
              Abre o explorador de arquivos em cada download.
            </p>
          </div>
          <Switch
            checked={preferences.downloadAsk}
            onCheckedChange={(value) =>
              setPreferences((current) => ({ ...current, downloadAsk: value }))
            }
            label="Perguntar onde salvar"
            disabled={loading}
          />
        </div>
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
          <div>
            <p className="flex items-center gap-1.5 text-[12.5px] font-medium text-zinc-300">
              <ShieldCheck size={13} className="text-amber-300" />
              Permitir scripts em HTML por padrão
            </p>
            <p className="text-[11.5px] text-zinc-500">
              Mesmo bloqueado por padrão, cada visualização pode liberar a
              execução uma vez.
            </p>
          </div>
          <Switch
            checked={preferences.htmlAllowScripts}
            onCheckedChange={(value) =>
              setPreferences((current) => ({
                ...current,
                htmlAllowScripts: value,
              }))
            }
            label="Permitir scripts em HTML"
            disabled={loading}
          />
        </div>
        <div className="flex justify-end">
          <Button
            size="sm"
            onClick={() => void savePreferences()}
            disabled={loading || saving}
          >
            {saving ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Save size={13} />
            )}
            Salvar preferências
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
