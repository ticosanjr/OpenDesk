import { useCallback, useEffect, useState } from "react";
import {
  AlertTriangle,
  Check,
  Loader2,
  MessageCircle,
  Play,
  Save,
  Send,
  Square,
  Trash2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { useAppStore } from "@/store/app-store";
import type { TelegramStatus } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  smallInputClass as inputClass,
  smallLabelClass as labelClass,
} from "@/lib/ui-classes";

export function TelegramCard() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [status, setStatus] = useState<TelegramStatus | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [token, setToken] = useState("");
  const [allowlist, setAllowlist] = useState("");
  const [workspaceId, setWorkspaceId] = useState("");
  const [agent, setAgent] = useState("build");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const value = await apiGet<TelegramStatus>("/api/integrations/telegram");
      setStatus(value);
      setEnabled(value.config.enabled);
      setAllowlist(value.config.allowlist.join(", "));
      setWorkspaceId(value.config.workspace_id ?? activeWorkspaceId ?? "");
      setAgent(value.config.agent || "build");
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [activeWorkspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function save(clearToken = false) {
    setBusy(clearToken ? "clear" : "save");
    setError(null);
    setMessage(null);
    try {
      const value = await apiPut<TelegramStatus>("/api/integrations/telegram", {
        enabled: clearToken ? false : enabled,
        token: token.trim() || null,
        clear_token: clearToken,
        allowlist: allowlist
          .split(/[,\n]/)
          .map((item) => item.trim())
          .filter(Boolean),
        workspace_id: workspaceId || null,
        agent: agent.trim() || "build",
      });
      setStatus(value);
      setEnabled(value.config.enabled);
      setToken("");
      if (clearToken) setEnabled(false);
      setMessage(
        clearToken
          ? "Token removido."
          : value.bridge_running
            ? "Configuração salva e bot em execução."
            : "Configuração salva. Inicie o bot quando token e allowlist estiverem prontos.",
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function toggleBridge(start: boolean) {
    setBusy(start ? "start" : "stop");
    setError(null);
    setMessage(null);
    try {
      const value = await apiPost<TelegramStatus>(
        `/api/integrations/telegram/${start ? "start" : "stop"}`,
      );
      setStatus(value);
      if (start) setEnabled(value.config.enabled);
      setMessage(start ? "Bot iniciado." : "Bot parado.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function test() {
    setBusy("test");
    setError(null);
    setMessage(null);
    try {
      const value = await apiPost<{ sent: number; bot: { username?: string } }>(
        "/api/integrations/telegram/test",
      );
      setMessage(
        `Teste enviado para ${value.sent} chat(s)${value.bot.username ? ` via @${value.bot.username}` : ""}.`,
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  const tokenOk = status?.token_configured ?? false;
  const running = status?.bridge_running ?? false;
  const bot = status?.bot_username ? `@${status.bot_username}` : null;

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Send size={15} className="text-zinc-500" />
            Telegram (celular)
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Converse com um agente pelo app do Telegram. O OpenDesk usa a Bot API
            com long polling, sem webhook e sem expor sua máquina.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant={tokenOk ? "green" : "red"}>
            token {tokenOk ? "ok" : "ausente"}
          </Badge>
          <Badge variant={running ? "green" : "default"}>
            bot {running ? "ativo" : "parado"}
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <label className="flex items-center gap-2 text-[13px] text-zinc-400">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(event) => setEnabled(event.target.checked)}
            className="accent-blue-500"
          />
          habilitar integração Telegram
        </label>

        <div className="grid gap-2 md:grid-cols-2">
          <label className="md:col-span-2">
            <span className={labelClass}>
              Token do bot {tokenOk ? "(deixe vazio para manter o atual)" : ""}
            </span>
            <input
              value={token}
              onChange={(event) => setToken(event.target.value)}
              placeholder="123456:ABC-DEF..."
              type="password"
              autoComplete="off"
              className={cn(inputClass, "font-mono text-[12px]")}
            />
          </label>
          <label>
            <span className={labelClass}>Workspace padrão</span>
            <select
              value={workspaceId}
              onChange={(event) => setWorkspaceId(event.target.value)}
              className={inputClass}
            >
              <option value="">workspace ativo</option>
              {workspaces.map((workspace) => (
                <option key={workspace.id} value={workspace.id}>
                  {workspace.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className={labelClass}>Agente padrão</span>
            <input
              value={agent}
              onChange={(event) => setAgent(event.target.value)}
              placeholder="build"
              className={inputClass}
            />
          </label>
        </div>

        <label>
          <span className={labelClass}>
            Allowlist de chats (IDs ou @username, separados por vírgula) — obrigatória
          </span>
          <textarea
            value={allowlist}
            onChange={(event) => setAllowlist(event.target.value)}
            rows={2}
            placeholder="123456789, -1001234567890, @meu_usuario"
            className={cn(inputClass, "h-auto resize-y py-2 font-mono text-[12px]")}
          />
        </label>

        <div className="flex flex-wrap gap-2">
          <Button size="sm" onClick={() => void save()} disabled={busy !== null}>
            {busy === "save" ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
            Salvar
          </Button>
          {running ? (
            <Button variant="outline" size="sm" onClick={() => void toggleBridge(false)} disabled={busy !== null}>
              <Square size={12} />
              Parar bot
            </Button>
          ) : (
            <Button variant="outline" size="sm" onClick={() => void toggleBridge(true)} disabled={busy !== null}>
              <Play size={12} />
              Iniciar bot
            </Button>
          )}
          <Button variant="outline" size="sm" onClick={() => void test()} disabled={busy !== null || !tokenOk}>
            {busy === "test" ? <Loader2 size={13} className="animate-spin" /> : <Send size={13} />}
            Enviar teste
          </Button>
          {tokenOk ? (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void save(true)}
              disabled={busy !== null}
            >
              {busy === "clear" ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Trash2 size={13} />
              )}
              Remover token
            </Button>
          ) : null}
        </div>

        {bot || status?.bot_name ? (
          <p className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[12px] text-zinc-400">
            <MessageCircle size={12} className="mr-1 inline" />
            Bot conectado: {bot ?? status?.bot_name}
            {status?.bot_name && bot ? ` (${status.bot_name})` : ""}
          </p>
        ) : null}

        {status?.last_error ? (
          <p className="flex items-start gap-2 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-200">
            <AlertTriangle size={12} className="mt-0.5 shrink-0" />
            {status.last_error}
          </p>
        ) : null}

        {message ? (
          <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[12px] text-emerald-300">
            <Check size={11} className="mr-1 inline" />
            {message}
          </p>
        ) : null}
        {error ? (
          <p className="flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            <AlertTriangle size={12} className="mt-0.5 shrink-0" />
            {error}
          </p>
        ) : null}

        <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[11.5px] leading-relaxed text-zinc-500">
          <p className="font-medium text-zinc-300">Como conectar</p>
          <ol className="mt-1 list-decimal space-y-0.5 pl-4">
            <li>No Telegram, fale com <span className="font-mono">@BotFather</span> e crie um bot com <span className="font-mono">/newbot</span>.</li>
            <li>Cole o token acima, salve e inicie o bot.</li>
            <li>Envie <span className="font-mono">/start</span> para o seu bot pelo celular.</li>
            <li>Adicione o chat ID (ou seu @username) à allowlist e salve.</li>
          </ol>
          <p className="mt-1">
            Segurança: a allowlist é obrigatória; chats fora dela são descartados.
            Para notificações de rotinas, use um chat ID numérico já conhecido.
          </p>
        </div>
      </CardContent>
    </Card>
  );
}
