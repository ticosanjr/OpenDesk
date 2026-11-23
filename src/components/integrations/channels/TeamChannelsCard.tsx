import { useCallback, useEffect, useState } from "react";
import { Loader2, MessageSquare, QrCode, ShieldCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPut, apiPost } from "@/lib/api";
import { toast } from "@/store/toast-store";

interface DiscordConfig {
  enabled: boolean;
  workspace_id?: string | null;
  guild_allowlist: string[];
  channel_allowlist: string[];
  user_allowlist: string[];
  agent: string;
  notes: string;
}

interface SlackConfig {
  enabled: boolean;
  workspace_id?: string | null;
  channel_allowlist: string[];
  user_allowlist: string[];
  agent: string;
  notes: string;
}

interface QrConfig {
  enabled: boolean;
  workspace_id?: string | null;
  sidecar_command: string;
  allowlist: string[];
  agent: string;
  notes: string;
}

const EMPTY_DISCORD: DiscordConfig = {
  enabled: false,
  workspace_id: null,
  guild_allowlist: [],
  channel_allowlist: [],
  user_allowlist: [],
  agent: "build",
  notes: "",
};
const EMPTY_SLACK: SlackConfig = {
  enabled: false,
  workspace_id: null,
  channel_allowlist: [],
  user_allowlist: [],
  agent: "build",
  notes: "",
};
const EMPTY_QR: QrConfig = {
  enabled: false,
  workspace_id: null,
  sidecar_command: "",
  allowlist: [],
  agent: "build",
  notes: "",
};

function list(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function TeamChannelsCard({ workspaceId }: { workspaceId: string }) {
  const [discord, setDiscord] = useState(EMPTY_DISCORD);
  const [slack, setSlack] = useState(EMPTY_SLACK);
  const [qr, setQr] = useState(EMPTY_QR);
  const [discordRunning, setDiscordRunning] = useState(false);
  const [slackRunning, setSlackRunning] = useState(false);
  const [qrRunning, setQrRunning] = useState(false);
  const [discordToken, setDiscordToken] = useState("");
  const [slackAppToken, setSlackAppToken] = useState("");
  const [slackBotToken, setSlackBotToken] = useState("");
  const [qrPayload, setQrPayload] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [d, s, q] = await Promise.all([
        apiGet<{
          config: DiscordConfig;
          running: boolean;
          token_set: boolean;
        }>("/api/channels/discord"),
        apiGet<{
          config: SlackConfig;
          running: boolean;
          app_token_set: boolean;
          bot_token_set: boolean;
        }>("/api/channels/slack"),
        apiGet<{
          config: QrConfig;
          running: boolean;
          last_qr?: string | null;
          last_status?: string | null;
        }>("/api/channels/whatsapp-qr"),
      ]);
      setDiscord({ ...d.config, workspace_id: d.config.workspace_id ?? workspaceId });
      setSlack({ ...s.config, workspace_id: s.config.workspace_id ?? workspaceId });
      setQr({ ...q.config, workspace_id: q.config.workspace_id ?? workspaceId });
      setDiscordRunning(d.running);
      setSlackRunning(s.running);
      setQrRunning(q.running);
      setQrPayload(q.last_qr ?? null);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar canais de equipe",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!qrRunning) return;
    const handle = window.setInterval(() => {
      void apiGet<{
        running: boolean;
        last_qr?: string | null;
      }>("/api/channels/whatsapp-qr")
        .then((value) => {
          setQrPayload(value.last_qr ?? null);
          setQrRunning(value.running);
        })
        .catch(() => {});
    }, 3_000);
    return () => window.clearInterval(handle);
  }, [qrRunning]);

  async function saveDiscord() {
    setBusy("discord");
    try {
      const value = await apiPut<{ running: boolean }>("/api/channels/discord", {
        ...discord,
        token: discordToken || undefined,
      });
      setDiscordRunning(value.running);
      setDiscordToken("");
      toast({ tone: "success", title: "Discord salvo" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha no Discord",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function saveSlack() {
    setBusy("slack");
    try {
      const value = await apiPut<{ running: boolean }>("/api/channels/slack", {
        ...slack,
        app_token: slackAppToken || undefined,
        bot_token: slackBotToken || undefined,
      });
      setSlackRunning(value.running);
      setSlackAppToken("");
      setSlackBotToken("");
      toast({ tone: "success", title: "Slack salvo" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha no Slack",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function saveQr() {
    setBusy("qr");
    try {
      const value = await apiPut<{ running: boolean }>("/api/channels/whatsapp-qr", qr);
      setQrRunning(value.running);
      await load();
      toast({ tone: "success", title: "WhatsApp QR salvo" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha no WhatsApp QR",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function toggle(
    key: "discord" | "slack" | "qr",
    action: "start" | "stop",
  ) {
    setBusy(`${key}:${action}`);
    try {
      const path =
        key === "qr"
          ? `/api/channels/whatsapp-qr/${action}`
          : `/api/channels/${key}/${action}`;
      const value = await apiPost<{ running: boolean }>(path);
      if (key === "discord") setDiscordRunning(value.running);
      if (key === "slack") setSlackRunning(value.running);
      if (key === "qr") {
        setQrRunning(value.running);
        await load();
      }
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível alterar o canal",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  if (loading) {
    return (
      <Card>
        <CardContent className="flex items-center gap-2 p-4 text-[13px] text-zinc-500">
          <Loader2 size={14} className="animate-spin" /> Carregando canais…
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-3">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2">
            <MessageSquare size={16} /> Discord
            <Badge variant={discordRunning ? "green" : "default"}>
              {discordRunning ? "conectado" : "parado"}
            </Badge>
          </CardTitle>
          <div className="flex gap-1">
            <Button variant="ghost" size="sm" onClick={() => void toggle("discord", discordRunning ? "stop" : "start")}>
              {discordRunning ? "Parar" : "Iniciar"}
            </Button>
            <Button size="sm" disabled={busy === "discord"} onClick={() => void saveDiscord()}>
              {busy === "discord" ? <Loader2 size={13} className="animate-spin" /> : null} Salvar
            </Button>
          </div>
        </CardHeader>
        <CardContent className="grid gap-2 sm:grid-cols-2">
          <label className="flex items-center gap-2 text-[13px] text-zinc-300">
            <input
              type="checkbox"
              checked={discord.enabled}
              onChange={(event) => setDiscord({ ...discord, enabled: event.target.checked })}
            />
            habilitado
          </label>
          <input
            value={discord.agent}
            onChange={(event) => setDiscord({ ...discord, agent: event.target.value })}
            placeholder="agente"
            className={inputClass}
          />
          <input
            type="password"
            value={discordToken}
            onChange={(event) => setDiscordToken(event.target.value)}
            placeholder="bot token (deixe vazio para manter)"
            className={inputClass}
            autoComplete="new-password"
          />
          <input
            value={discord.guild_allowlist.join(", ")}
            onChange={(event) => setDiscord({ ...discord, guild_allowlist: list(event.target.value) })}
            placeholder="guild ids permitidos"
            className={inputClass}
          />
          <input
            value={discord.channel_allowlist.join(", ")}
            onChange={(event) => setDiscord({ ...discord, channel_allowlist: list(event.target.value) })}
            placeholder="channel ids permitidos"
            className={inputClass}
          />
          <input
            value={discord.user_allowlist.join(", ")}
            onChange={(event) => setDiscord({ ...discord, user_allowlist: list(event.target.value) })}
            placeholder="user ids permitidos"
            className={inputClass}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2">
            <ShieldCheck size={16} /> Slack
            <Badge variant={slackRunning ? "green" : "default"}>
              {slackRunning ? "conectado" : "parado"}
            </Badge>
          </CardTitle>
          <div className="flex gap-1">
            <Button variant="ghost" size="sm" onClick={() => void toggle("slack", slackRunning ? "stop" : "start")}>
              {slackRunning ? "Parar" : "Iniciar"}
            </Button>
            <Button size="sm" disabled={busy === "slack"} onClick={() => void saveSlack()}>
              {busy === "slack" ? <Loader2 size={13} className="animate-spin" /> : null} Salvar
            </Button>
          </div>
        </CardHeader>
        <CardContent className="grid gap-2 sm:grid-cols-2">
          <label className="flex items-center gap-2 text-[13px] text-zinc-300">
            <input
              type="checkbox"
              checked={slack.enabled}
              onChange={(event) => setSlack({ ...slack, enabled: event.target.checked })}
            />
            habilitado
          </label>
          <input
            value={slack.agent}
            onChange={(event) => setSlack({ ...slack, agent: event.target.value })}
            placeholder="agente"
            className={inputClass}
          />
          <input
            type="password"
            value={slackAppToken}
            onChange={(event) => setSlackAppToken(event.target.value)}
            placeholder="app token (xapp-...)"
            className={inputClass}
            autoComplete="new-password"
          />
          <input
            type="password"
            value={slackBotToken}
            onChange={(event) => setSlackBotToken(event.target.value)}
            placeholder="bot token (xoxb-...)"
            className={inputClass}
            autoComplete="new-password"
          />
          <input
            value={slack.channel_allowlist.join(", ")}
            onChange={(event) => setSlack({ ...slack, channel_allowlist: list(event.target.value) })}
            placeholder="canais permitidos"
            className={inputClass}
          />
          <input
            value={slack.user_allowlist.join(", ")}
            onChange={(event) => setSlack({ ...slack, user_allowlist: list(event.target.value) })}
            placeholder="usuários permitidos"
            className={inputClass}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2">
            <QrCode size={16} /> WhatsApp Web (QR)
            <Badge variant={qrRunning ? "green" : "default"}>
              {qrRunning ? "conectado" : "parado"}
            </Badge>
          </CardTitle>
          <div className="flex gap-1">
            <Button variant="ghost" size="sm" onClick={() => void toggle("qr", qrRunning ? "stop" : "start")}>
              {qrRunning ? "Parar" : "Iniciar"}
            </Button>
            <Button size="sm" disabled={busy === "qr"} onClick={() => void saveQr()}>
              {busy === "qr" ? <Loader2 size={13} className="animate-spin" /> : null} Salvar
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-2">
          <div className="grid gap-2 sm:grid-cols-2">
            <label className="flex items-center gap-2 text-[13px] text-zinc-300">
              <input
                type="checkbox"
                checked={qr.enabled}
                onChange={(event) => setQr({ ...qr, enabled: event.target.checked })}
              />
              habilitado
            </label>
            <input
              value={qr.agent}
              onChange={(event) => setQr({ ...qr, agent: event.target.value })}
              placeholder="agente"
              className={inputClass}
            />
            <input
              value={qr.sidecar_command}
              onChange={(event) => setQr({ ...qr, sidecar_command: event.target.value })}
              placeholder="comando do sidecar (ex.: node ~/wweb/bridge.js)"
              className={`${inputClass} sm:col-span-2`}
            />
            <input
              value={qr.allowlist.join(", ")}
              onChange={(event) => setQr({ ...qr, allowlist: list(event.target.value) })}
              placeholder="números permitidos (5511...)"
              className={`${inputClass} sm:col-span-2`}
            />
          </div>
          <p className="text-[11.5px] text-zinc-500">
            O sidecar é externo e fala JSON Lines via stdio. O OpenDesk não instala nem
            embute Baileys.
          </p>
          {qrPayload ? (
            qrPayload.startsWith("data:image") ? (
              <img src={qrPayload} alt="QR do WhatsApp" className="h-48 w-48 bg-white p-2" />
            ) : (
              <code className="block break-all rounded border border-zinc-800 bg-zinc-950/60 p-2 font-mono text-[11px] text-zinc-400">
                {qrPayload}
              </code>
            )
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60";
