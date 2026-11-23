import { useCallback, useEffect, useState } from "react";
import { CheckCircle2, Copy, ExternalLink, Globe, Loader2, Power, RefreshCw } from "lucide-react";
import { openExternalUrl } from "@/lib/runtime-env";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPost } from "@/lib/api";
import type { BrowserStatus } from "@/lib/types";
import { toast } from "@/store/toast-store";
import { useAppStore } from "@/store/app-store";
import { cn } from "@/lib/utils";
import { fieldLabelClass as labelClass } from "@/lib/ui-classes";

export function BrowserCard() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [scope, setScope] = useState<"workspace" | "global">("workspace");
  const [status, setStatus] = useState<BrowserStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const query = activeWorkspaceId
    ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}&scope=${scope}`
    : `?scope=${scope}`;

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setStatus(null);
      return;
    }
    setLoading(true);
    try {
      setStatus(await apiGet<BrowserStatus>(`/api/browse/status${query}`));
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível ler o estado do Browse",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    void load();
  }, [load]);

  async function toggle(enabled: boolean) {
    if (!activeWorkspaceId) return;
    setBusy(enabled ? "enable" : "disable");
    try {
      const next = await apiPost<BrowserStatus>(
        `/api/browse/${enabled ? "enable" : "disable"}`,
        { workspace_id: activeWorkspaceId, scope },
      );
      setStatus(next);
      toast({
        tone: "success",
        title: enabled ? "Browse ativado" : "Browse desativado",
        description: enabled
          ? "MCP e skill de navegação gravados no workspace."
          : "A entrada MCP foi removida; a skill foi preservada.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível alterar o Browse",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function copyCommand() {
    if (!status?.command.length) return;
    try {
      await navigator.clipboard.writeText(status.command.join(" "));
      toast({ tone: "success", title: "Comando copiado" });
    } catch {
      toast({ tone: "info", title: status.command.join(" ") });
    }
  }

  return (
    <Card id="integrations-browser">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Globe size={15} className="text-sky-300" />
            OpenCode Browse
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Navegador real do usuário via MCP, com abas, logins, screenshots e
            handoff para 2FA. Tarefas marcadas como “Precisa navegar na internet”
            ativam esta integração automaticamente.
          </p>
        </div>
        <Badge variant={status?.configured ? "green" : "default"}>
          {status?.configured ? "MCP ativo" : "não configurado"}
        </Badge>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-2 sm:grid-cols-3">
          <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
            <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">
              Binário
            </p>
            <p className="mt-0.5 flex items-center gap-1 text-[12px] text-zinc-300">
              {status?.binary ? (
                <>
                  <CheckCircle2 size={11} className="text-emerald-400" />
                  detectado
                </>
              ) : (
                "usará npx @opencode/browse"
              )}
            </p>
          </div>
          <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
            <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">
              MCP no OpenCode
            </p>
            <p className="mt-0.5 text-[12px] text-zinc-300">
              {status?.configured ? "browse · local" : "ausente"}
            </p>
          </div>
          <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
            <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">
              Skill de navegação
            </p>
            <p className="mt-0.5 text-[12px] text-zinc-300">
              {status?.skill ? "browse-web" : "não criada"}
            </p>
          </div>
        </div>

        <div className="grid gap-2 sm:grid-cols-[180px_1fr]">
          <label>
            <span className={labelClass}>Escopo</span>
            <select
              value={scope}
              onChange={(event) => setScope(event.target.value as "workspace" | "global")}
              className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[13.5px] text-zinc-100 outline-none"
            >
              <option value="workspace">Workspace</option>
              <option value="global">Global</option>
            </select>
          </label>
          <div>
            <span className={labelClass}>Comando MCP</span>
            <div className="flex items-center gap-1.5">
              <code className="min-w-0 flex-1 truncate rounded-lg border border-zinc-800 bg-zinc-950/50 px-2.5 py-2 font-mono text-[12px] text-zinc-400">
                {status?.command.join(" ") ?? "—"}
              </code>
              <Button
                variant="outline"
                size="sm"
                className="h-9"
                onClick={() => void copyCommand()}
                aria-label="Copiar comando MCP"
              >
                <Copy size={12} />
              </Button>
            </div>
          </div>
        </div>

        <p className="text-[12px] leading-relaxed text-zinc-500">
          {status?.install_hint ?? ""} A extensão Chromium é obrigatória para o
          agente controlar o navegador. Login, 2FA, pagamento e captcha são
          entregues ao usuário por handoff; o OpenDesk não contorna essas etapas.
        </p>

        <div className="flex flex-wrap items-center justify-end gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              void openExternalUrl("https://opencode.ai/product/browse").catch(() =>
                toast({ tone: "error", title: "Não foi possível abrir o link" }),
              )
            }
          >
            <ExternalLink size={12} />
            Instalar extensão
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void load()}
            disabled={loading}
          >
            <RefreshCw size={12} className={loading ? "animate-spin" : undefined} />
            Atualizar
          </Button>
          {status?.configured ? (
            <Button
              variant="outline"
              size="sm"
              className="border-red-500/30 text-red-300"
              disabled={busy !== null}
              onClick={() => void toggle(false)}
            >
              {busy === "disable" ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Power size={12} />
              )}
              Desativar
            </Button>
          ) : (
            <Button
              size="sm"
              disabled={busy !== null || !activeWorkspaceId}
              onClick={() => void toggle(true)}
            >
              {busy === "enable" ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Power size={12} />
              )}
              Ativar Browse
            </Button>
          )}
        </div>

        <p className={cn("text-[11.5px] text-zinc-500", !status?.config_path && "hidden")}>
          Config: {status?.config_path}
        </p>
      </CardContent>
    </Card>
  );
}
