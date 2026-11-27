import { useState } from "react";
import { Loader2, Settings2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { apiPost, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";
import type { ChannelBot } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  smallInputClass as inputClass,
  smallLabelClass as labelClass,
} from "@/lib/ui-classes";

export type Channel = "telegram" | "signal" | "whatsapp";

export interface BotDraft {
  name: string;
  token: string;
  allowlist: string;
  defaultAgent: string;
  enabled: boolean;
  account: string;
  cliPath: string;
}

const LABEL: Record<Channel, string> = {
  telegram: "Telegram",
  signal: "Signal",
  whatsapp: "WhatsApp",
};

function parseConfig(bot: ChannelBot): { account?: string; cli_path?: string } {
  try {
    return JSON.parse(bot.config_json || "{}") as {
      account?: string;
      cli_path?: string;
    };
  } catch {
    return {};
  }
}

export function emptyDraft(channel: Channel): BotDraft {
  return {
    name: channel === "signal" ? "Signal principal" : `Bot ${LABEL[channel]}`,
    token: "",
    allowlist: "",
    defaultAgent: "build",
    enabled: false,
    account: "",
    cliPath: "signal-cli",
  };
}

export function draftFromBot(bot: ChannelBot): BotDraft {
  const config = parseConfig(bot);
  return {
    name: bot.name,
    token: "",
    allowlist: bot.allowlist.join(", "),
    defaultAgent: bot.default_agent || "build",
    enabled: bot.enabled,
    account: config.account ?? "",
    cliPath: config.cli_path || "signal-cli",
  };
}

function splitList(value: string): string[] {
  return value
    .split(/[,\n]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

export function ChannelBotForm({
  channel,
  workspaceId,
  bot,
  onCancel,
  onSaved,
}: {
  channel: Channel;
  workspaceId: string;
  bot: ChannelBot | null;
  onCancel: () => void;
  onSaved: () => void;
}) {
  const [draft, setDraft] = useState<BotDraft>(() =>
    bot ? draftFromBot(bot) : emptyDraft(channel),
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const editing = bot !== null;
  const label = LABEL[channel];

  function patch(value: Partial<BotDraft>) {
    setDraft((current) => ({ ...current, ...value }));
  }

  async function save() {
    if (!draft.name.trim()) {
      setError("Informe um nome para o bot.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const allowlist = splitList(draft.allowlist);
      const config =
        channel === "signal"
          ? JSON.stringify({
              enabled: true,
              account: draft.account.trim(),
              cli_path: draft.cliPath.trim() || "signal-cli",
              allowlist,
              workspace_id: workspaceId,
              agent: draft.defaultAgent.trim() || "build",
            })
          : null;
      const body = {
        workspace_id: workspaceId,
        channel,
        name: draft.name.trim(),
        enabled: draft.enabled,
        token: draft.token.trim() || null,
        allowlist,
        default_agent: draft.defaultAgent.trim() || "build",
        config_json: config,
      };
      if (bot) {
        await apiPut(`/api/channels/bots/${bot.id}`, body);
        toast({ tone: "success", title: "Bot atualizado" });
      } else {
        await apiPost("/api/channels/bots", body);
        toast({ tone: "success", title: "Bot criado" });
      }
      onSaved();
    } catch (cause) {
      const description = cause instanceof Error ? cause.message : String(cause);
      setError(description);
      toast({ tone: "error", title: "Falha ao salvar o bot", description });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-2 rounded-lg border border-blue-500/20 bg-blue-500/5 p-3">
      <div className="grid gap-2 md:grid-cols-2">
        <label>
          <span className={labelClass}>Nome do bot</span>
          <input
            value={draft.name}
            onChange={(event) => patch({ name: event.target.value })}
            className={inputClass}
            placeholder={`Bot ${label}`}
          />
        </label>
        <label>
          <span className={labelClass}>Agente padrão</span>
          <input
            value={draft.defaultAgent}
            onChange={(event) => patch({ defaultAgent: event.target.value })}
            className={inputClass}
            placeholder="build"
          />
        </label>
        {channel === "telegram" ? (
          <label className="md:col-span-2">
            <span className={labelClass}>
              Token do bot {editing ? "(deixe vazio para manter o atual)" : ""}
            </span>
            <input
              value={draft.token}
              onChange={(event) => patch({ token: event.target.value })}
              type="password"
              autoComplete="off"
              className={cn(inputClass, "font-mono text-[12px]")}
              placeholder="123456:ABC-DEF..."
            />
          </label>
        ) : null}
        {channel === "signal" ? (
          <>
            <label>
              <span className={labelClass}>Número da conta</span>
              <input
                value={draft.account}
                onChange={(event) => patch({ account: event.target.value })}
                className={inputClass}
                placeholder="+5511999999999"
              />
            </label>
            <label>
              <span className={labelClass}>Caminho do signal-cli</span>
              <input
                value={draft.cliPath}
                onChange={(event) => patch({ cliPath: event.target.value })}
                className={cn(inputClass, "font-mono text-[12px]")}
                placeholder="signal-cli"
              />
            </label>
          </>
        ) : null}
      </div>
      <label>
        <span className={labelClass}>
          Allowlist (IDs ou @username, separados por vírgula)
        </span>
        <textarea
          value={draft.allowlist}
          onChange={(event) => patch({ allowlist: event.target.value })}
          rows={2}
          className={cn(inputClass, "h-auto resize-y py-2 font-mono text-[12px]")}
          placeholder="-1001234567890, @meu_usuario"
        />
      </label>
      <label className="flex items-center gap-2 text-[12.5px] text-zinc-400">
        <Switch
          checked={draft.enabled}
          onCheckedChange={(value) => patch({ enabled: value })}
          label="Iniciar automaticamente com o OpenDesk"
        />
        Iniciar automaticamente com o OpenDesk
      </label>
      <div className="flex flex-wrap gap-2">
        <Button size="sm" onClick={() => void save()} disabled={busy}>
          {busy ? (
            <Loader2 size={13} className="animate-spin" />
          ) : (
            <Settings2 size={13} />
          )}
          Salvar bot
        </Button>
        <Button variant="ghost" size="sm" onClick={onCancel} disabled={busy}>
          Cancelar
        </Button>
      </div>
      {editing ? (
        <p className="text-[11.5px] text-zinc-500">
          O token atual nunca é exibido. Preencha o campo apenas para substituí-lo.
        </p>
      ) : null}
      {error ? (
        <p className="text-[12px] text-red-300">{error}</p>
      ) : null}
    </div>
  );
}
