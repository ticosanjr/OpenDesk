import { useCallback, useEffect, useState } from "react";
import { CheckCircle2, Download, Loader2, Package, XCircle } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { apiGet, apiPost } from "@/lib/api";
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
  manual_hint: string;
}

export function ToolsCard() {
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);

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

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Package size={15} className="text-zinc-500" />
            Ferramentas externas
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Binários opcionais ficam em <span className="font-mono">~/.local/share/opendesk/tools</span> e
            sobrevivem às atualizações do OpenDesk.
          </p>
        </div>
        <Badge variant="default">{tools.filter((tool) => tool.installed).length} instaladas</Badge>
      </CardHeader>
      <CardContent className="space-y-2">
        {loading ? (
          <p className="flex items-center gap-2 p-3 text-[11px] text-zinc-500">
            <Loader2 size={12} className="animate-spin" /> Detectando…
          </p>
        ) : (
          tools.map((tool) => (
            <div
              key={tool.name}
              className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
            >
              {tool.installed ? (
                <CheckCircle2 size={14} className="text-emerald-400" />
              ) : (
                <XCircle size={14} className="text-zinc-600" />
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
                >
                  {installing === tool.name ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <Download size={12} />
                  )}
                  Baixar
                </Button>
              ) : null}
            </div>
          ))
        )}
      </CardContent>
    </Card>
  );
}
