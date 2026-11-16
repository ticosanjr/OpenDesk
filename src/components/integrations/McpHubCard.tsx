import { useCallback, useEffect, useState } from "react";
import { Boxes, Loader2, PlugZap, RefreshCw, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { toast } from "@/store/toast-store";

interface HubServer {
  name: string;
  scope: string;
  workspace_id: string;
  transport: string;
  disabled: boolean;
  command?: string | null;
  url?: string | null;
  health: string;
  detail?: string | null;
}

interface CatalogEntry {
  id: string;
  name: string;
  description: string;
  transport: string;
}

function healthTone(health: string): "green" | "red" | "yellow" | "default" {
  if (["ready", "connected", "connected-disabled", "ok", "reachable"].includes(health)) {
    return "green";
  }
  if (["missing", "invalid", "failed", "error", "unreachable"].includes(health)) {
    return "red";
  }
  if (health === "disabled") return "default";
  return "yellow";
}

export function McpHubCard({ workspaceId }: { workspaceId: string }) {
  const [servers, setServers] = useState<HubServer[]>([]);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);

  const query = `?workspace_id=${encodeURIComponent(workspaceId)}`;

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      const [inventory, entries] = await Promise.all([
        apiGet<HubServer[]>(`/api/mcp-hub/servers${query}`),
        apiGet<CatalogEntry[]>("/api/mcp-hub/catalog"),
      ]);
      setServers(inventory);
      setCatalog(entries);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar o MCP Hub",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [workspaceId, query]);

  useEffect(() => {
    void load();
  }, [load]);

  async function install(entry: CatalogEntry) {
    setBusy(`install:${entry.id}`);
    try {
      await apiPost("/api/mcp-hub/install", {
        catalog_id: entry.id,
        scope: "workspace",
        workspace_id: workspaceId,
      });
      await load();
      toast({
        tone: "success",
        title: `${entry.name} instalado`,
        description: "Configuração gravada em opencode.jsonc nativo.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao instalar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function toggle(server: HubServer) {
    setBusy(`toggle:${server.scope}:${server.name}`);
    try {
      await apiPost(
        `/api/mcp-hub/${server.scope}/${encodeURIComponent(server.name)}/toggle${query}`,
        { disabled: !server.disabled },
      );
      await load();
    } finally {
      setBusy(null);
    }
  }

  async function test(server: HubServer) {
    setBusy(`test:${server.scope}:${server.name}`);
    try {
      const result = await apiPost<{ status: string; detail?: string }>(
        `/api/mcp-hub/${server.scope}/${encodeURIComponent(server.name)}/test${query}`,
      );
      toast({
        tone: result.status === "missing" || result.status === "unreachable" ? "error" : "success",
        title: `${server.name}: ${result.status}`,
        description: result.detail,
      });
      await load();
    } finally {
      setBusy(null);
    }
  }

  async function remove(server: HubServer) {
    setBusy(`delete:${server.scope}:${server.name}`);
    try {
      await apiDelete(
        `/api/mcp-hub/${server.scope}/${encodeURIComponent(server.name)}`,
      );
      await load();
    } finally {
      setBusy(null);
    }
  }

  if (loading) {
    return (
      <Card>
        <CardContent className="flex items-center gap-2 py-6 text-[13px] text-zinc-500">
          <Loader2 size={14} className="animate-spin" />
          carregando MCP Hub…
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <PlugZap size={15} className="text-zinc-500" />
              MCP Hub
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Inventário unificado de <span className="font-mono">mcp.servers</span>{" "}
              (global + workspace), com catálogo, teste e liga/desliga. A fonte
              continua sendo o <span className="font-mono">opencode.jsonc</span>.
            </p>
          </div>
          <Button variant="outline" size="sm" className="h-7" onClick={() => void load()}>
            <RefreshCw size={11} />
            Atualizar
          </Button>
        </CardHeader>
        <CardContent className="space-y-3">
          {servers.length === 0 ? (
            <EmptyState
              title="Nenhum servidor MCP"
              description="Instale um item do catálogo ou configure no Studio."
            />
          ) : (
            <div className="space-y-1.5">
              {servers.map((server) => {
                const key = `${server.scope}:${server.name}`;
                return (
                  <div
                    key={key}
                    className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
                  >
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <span className="font-mono text-[12.5px] text-zinc-200">
                          {server.name}
                        </span>
                        <Badge variant={server.scope === "global" ? "purple" : "blue"}>
                          {server.scope}
                        </Badge>
                        <Badge variant="default">{server.transport}</Badge>
                        <Badge variant={healthTone(server.health)}>{server.health}</Badge>
                      </div>
                      <p className="mt-0.5 truncate font-mono text-[11px] text-zinc-600">
                        {server.command ?? server.url ?? "—"}
                      </p>
                      {server.detail ? (
                        <p className="text-[11px] text-zinc-500">{server.detail}</p>
                      ) : null}
                    </div>
                    <div className="flex items-center gap-1.5">
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        disabled={busy === `test:${key}` || server.disabled}
                        onClick={() => void test(server)}
                      >
                        Testar
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        disabled={busy === `toggle:${key}`}
                        onClick={() => void toggle(server)}
                      >
                        {server.disabled ? "Ativar" : "Desativar"}
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2"
                        disabled={busy === `delete:${key}`}
                        onClick={() => void remove(server)}
                      >
                        <Trash2 size={11} />
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Boxes size={15} className="text-zinc-500" />
              Catálogo
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Instalação no escopo do workspace. Comandos <span className="font-mono">npx</span>{" "}
              exigem Node.js no PATH.
            </p>
          </div>
        </CardHeader>
        <CardContent>
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
            {catalog.map((entry) => (
              <div
                key={entry.id}
                className="flex flex-col justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3"
              >
                <div>
                  <div className="flex items-center gap-1.5">
                    <span className="text-[13px] font-medium text-zinc-200">
                      {entry.name}
                    </span>
                    <Badge variant="default">{entry.transport}</Badge>
                  </div>
                  <p className="mt-1 text-[11.5px] leading-relaxed text-zinc-500">
                    {entry.description}
                  </p>
                </div>
                <Button
                  size="sm"
                  className="h-7 self-start"
                  disabled={busy === `install:${entry.id}`}
                  onClick={() => void install(entry)}
                >
                  Instalar
                </Button>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
