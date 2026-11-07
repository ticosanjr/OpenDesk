import { useCallback, useEffect, useState } from "react";
import {
  CheckCircle2,
  Download,
  FileSearch,
  Loader2,
  Package,
  RefreshCw,
  XCircle,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { IconLabel } from "@/components/ui/icon-label";
import { LoadingState } from "@/components/ui/loading-state";
import { Tooltip } from "@/components/ui/tooltip";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { pickFile } from "@/lib/folder";
import { toast } from "@/store/toast-store";

interface ToolInfo {
  name: string;
  label: string;
  purpose: string;
  version: string;
  optional: boolean;
  installed: boolean;
  managed: boolean;
  path?: string | null;
  detected_version?: string | null;
  download_url?: string | null;
  checksum?: string | null;
  platform_supported: boolean;
  manual_hint: string;
}

export function ToolsCard() {
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);
  const [manualPath, setManualPath] = useState<Record<string, string>>({});

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setTools(await apiGet<ToolInfo[]>("/api/io/tools"));
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível listar ferramentas",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function install(tool: ToolInfo) {
    setInstalling(tool.name);
    try {
      await apiPost(`/api/io/tools/${tool.name}/install`, {});
      await load();
      toast({ tone: "success", title: `${tool.label} instalado` });
    } catch (cause) {
      toast({
        tone: "error",
        title: `Falha ao instalar ${tool.label}`,
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setInstalling(null);
    }
  }

  async function choosePath(tool: ToolInfo) {
    const selected = await pickFile(`Selecionar binário do ${tool.label}`);
    if (!selected) return;
    try {
      await apiPut(`/api/io/tools/${tool.name}/path`, { path: selected });
      setManualPath((current) => ({ ...current, [tool.name]: "" }));
      await load();
      toast({ tone: "success", title: `${tool.label} configurado`, description: selected });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Caminho inválido",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function clearPath(tool: ToolInfo) {
    try {
      await apiPut(`/api/io/tools/${tool.name}/path`, { path: null });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível remover o caminho",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  const installedCount = tools.filter((tool) => tool.installed).length;

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Package size={15} className="text-zinc-500" />
            Ferramentas externas
            <CountPill value={`${installedCount}/${tools.length}`} tone="green" />
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Typst, Tesseract e Whisper.cpp são opcionais. O status é detectado
            no sistema e os binários gerenciados ficam em{" "}
            <span className="font-mono">~/.local/share/opendesk/tools</span>.
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
          <IconLabel icon={<RefreshCw size={13} className={loading ? "animate-spin" : undefined} />}>
            Verificar novamente
          </IconLabel>
        </Button>
      </CardHeader>
      <CardContent className="space-y-2">
        {loading ? (
          <LoadingState compact rows={3} label="Detectando ferramentas instaladas…" />
        ) : (
          tools.map((tool) => (
            <div
              key={tool.name}
              className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
            >
              <div className="flex flex-wrap items-center gap-2">
                {tool.installed ? (
                  <CheckCircle2 size={14} className="shrink-0 text-emerald-400" />
                ) : (
                  <XCircle size={14} className="shrink-0 text-zinc-600" />
                )}
                <div className="min-w-0 flex-1">
                  <p className="text-[12px] text-zinc-200">{tool.label}</p>
                  <p className="truncate text-[9px] text-zinc-600">{tool.purpose}</p>
                  <p className="truncate font-mono text-[9px] text-zinc-600">
                    {tool.detected_version ?? tool.path ?? tool.manual_hint}
                  </p>
                </div>
                <Badge variant={tool.managed ? "blue" : tool.installed ? "green" : "default"}>
                  {tool.managed ? "gerenciado" : tool.installed ? "sistema" : "ausente"}
                </Badge>
                {!tool.installed && tool.download_url ? (
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7"
                    disabled={installing === tool.name}
                    onClick={() => void install(tool)}
                    title={
                      tool.checksum
                        ? `Downloads verificados por SHA256: ${tool.checksum}`
                        : "Baixar e instalar localmente"
                    }
                  >
                    <IconLabel
                      icon={
                        installing === tool.name ? (
                          <Loader2 size={12} className="animate-spin" />
                        ) : (
                          <Download size={12} />
                        )
                      }
                    >
                      Baixar
                    </IconLabel>
                  </Button>
                ) : null}
                {!tool.installed && !tool.download_url ? (
                  <Tooltip content={tool.manual_hint}>
                    <Badge variant="yellow">instalação manual</Badge>
                  </Tooltip>
                ) : null}
                {!tool.installed ? (
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-7"
                    onClick={() => void choosePath(tool)}
                    title="Escolher o binário já instalado no sistema"
                  >
                    <IconLabel icon={<FileSearch size={12} />}>Usar caminho</IconLabel>
                  </Button>
                ) : tool.managed ? (
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-7"
                    onClick={() => void clearPath(tool)}
                    title="Remover o caminho manual"
                  >
                    remover caminho
                  </Button>
                ) : null}
              </div>
              {!tool.installed ? (
                <p className="mt-1 text-[9px] leading-snug text-zinc-500">
                  {tool.download_url
                    ? "Baixe pelo OpenDesk ou aponte para um binário existente."
                    : tool.manual_hint}
                </p>
              ) : null}
              {manualPath[tool.name] ? (
                <p className="mt-1 font-mono text-[9px] text-zinc-500">
                  {manualPath[tool.name]}
                </p>
              ) : null}
            </div>
          ))
        )}
      </CardContent>
    </Card>
  );
}
