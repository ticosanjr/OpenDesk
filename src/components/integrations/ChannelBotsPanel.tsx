import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  Bot,
  Loader2,
  MessageSquare,
  Plus,
  RefreshCw,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import {
  ChannelBotForm,
  type Channel,
} from "@/components/integrations/ChannelBotForm";
import { ChannelBotRow } from "@/components/integrations/ChannelBotRow";
import { ChannelBotContextsModal } from "@/components/integrations/ChannelBotContextsModal";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { confirmAction } from "@/store/confirm-store";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import type { ChannelBot, ChannelChatContext } from "@/lib/types";

const CHANNEL_LABEL: Record<Channel, string> = {
  telegram: "Telegram",
  signal: "Signal",
  whatsapp: "WhatsApp",
};

export function ChannelBotsPanel({
  channel,
  workspaceId,
}: {
  channel: Channel;
  workspaceId: string;
}) {
  const workspaces = useAppStore((state) => state.workspaces);
  const [bots, setBots] = useState<ChannelBot[]>([]);
  const [otherBots, setOtherBots] = useState<ChannelBot[]>([]);
  const [counts, setCounts] = useState<Record<string, number>>({});
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [formBot, setFormBot] = useState<ChannelBot | null>(null);
  const [formOpen, setFormOpen] = useState(false);
  const [contextsBot, setContextsBot] = useState<ChannelBot | null>(null);
  const [error, setError] = useState<string | null>(null);

  const workspaceName =
    workspaces.find((workspace) => workspace.id === workspaceId)?.name ??
    "workspace ativo";
  const label = CHANNEL_LABEL[channel];

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    setError(null);
    try {
      const list = await apiGet<ChannelBot[]>(
        `/api/channels/bots?all=true&channel=${encodeURIComponent(channel)}`,
      );
      setBots(list.filter((bot) => bot.workspace_id === workspaceId));
      setOtherBots(list.filter((bot) => bot.workspace_id !== workspaceId));
      const entries = await Promise.all(
        list.map(async (bot) => {
          try {
            const contexts = await apiGet<ChannelChatContext[]>(
              `/api/channels/bots/${bot.id}/contexts`,
            );
            return [bot.id, contexts.length] as const;
          } catch {
            return [bot.id, 0] as const;
          }
        }),
      );
      setCounts(Object.fromEntries(entries));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [channel, workspaceId]);

  useEffect(() => {
    setFormOpen(false);
    setFormBot(null);
    setContextsBot(null);
    void load();
  }, [load, workspaceId]);

  const othersByWorkspace = useMemo(() => {
    const groups = new Map<string, ChannelBot[]>();
    for (const bot of otherBots) {
      groups.set(bot.workspace_id, [...(groups.get(bot.workspace_id) ?? []), bot]);
    }
    return [...groups.entries()];
  }, [otherBots]);

  function startCreate() {
    setFormBot(null);
    setFormOpen(true);
  }

  function startEdit(bot: ChannelBot) {
    setFormBot(bot);
    setFormOpen(true);
  }

  async function toggle(bot: ChannelBot, start: boolean) {
    setBusy(`${start ? "start" : "stop"}:${bot.id}`);
    setError(null);
    try {
      await apiPost(
        `/api/channels/bots/${bot.id}/${start ? "start" : "stop"}`,
      );
      toast({
        tone: "success",
        title: start ? "Bot iniciado" : "Bot parado",
        description: bot.name,
      });
      await load();
    } catch (cause) {
      const description = cause instanceof Error ? cause.message : String(cause);
      setError(description);
      toast({ tone: "error", title: "Falha ao controlar o bot", description });
    } finally {
      setBusy(null);
    }
  }

  async function test(bot: ChannelBot) {
    setBusy(`test:${bot.id}`);
    setError(null);
    try {
      const result = await apiPost<{ sent: number }>(
        `/api/channels/bots/${bot.id}/test`,
      );
      toast({
        tone: "success",
        title: "Teste enviado",
        description: `${result.sent} chat(s) receberam a mensagem.`,
      });
    } catch (cause) {
      const description = cause instanceof Error ? cause.message : String(cause);
      setError(description);
      toast({ tone: "error", title: "Teste falhou", description });
    } finally {
      setBusy(null);
    }
  }

  async function remove(bot: ChannelBot) {
    const confirmed = await confirmAction({
      title: `Excluir ${bot.name}?`,
      description:
        "O token do bot e os contextos por grupo serão removidos. Esta ação não pode ser desfeita.",
      confirmLabel: "Excluir bot",
      tone: "destructive",
    });
    if (!confirmed) return;
    setBusy(`delete:${bot.id}`);
    try {
      await apiDelete(`/api/channels/bots/${bot.id}`);
      toast({ tone: "success", title: "Bot excluído" });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao excluir",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Bot size={15} className="text-zinc-500" />
            Bots de {label}
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Workspace <span className="text-zinc-300">{workspaceName}</span>. Cada bot
            mantém sessões e contextos isolados por grupo ou conversa.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant={bots.length > 0 ? "blue" : "default"}>
            {bots.length} bot(s)
          </Badge>
          {otherBots.length > 0 ? (
            <Badge variant="default">
              {otherBots.length} em outras workspaces
            </Badge>
          ) : null}
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" onClick={startCreate} disabled={busy !== null}>
            <Plus size={13} />
            Novo bot
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void load()}
            disabled={loading || busy !== null}
          >
            {loading ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <RefreshCw size={13} />
            )}
            Atualizar
          </Button>
        </div>

        {formOpen ? (
          <ChannelBotForm
            key={formBot?.id ?? "new"}
            channel={channel}
            workspaceId={workspaceId}
            bot={formBot}
            onCancel={() => setFormOpen(false)}
            onSaved={() => {
              setFormOpen(false);
              setFormBot(null);
              void load();
            }}
          />
        ) : null}

        {loading && bots.length === 0 ? (
          <p className="flex items-center gap-2 text-[12.5px] text-zinc-400">
            <Loader2 size={14} className="animate-spin" />
            Carregando bots…
          </p>
        ) : null}

        {!loading && bots.length === 0 ? (
          <EmptyState
            compact
            icon={MessageSquare}
            title={`Nenhum bot de ${label} neste workspace`}
            description="Crie um bot com token próprio para isolar grupos e conversas deste workspace."
            action={
              <Button size="sm" variant="outline" onClick={startCreate}>
                <Plus size={13} />
                Criar primeiro bot
              </Button>
            }
          />
        ) : null}

        <div className="space-y-2">
          {bots.map((bot) => (
            <ChannelBotRow
              key={bot.id}
              bot={bot}
              count={counts[bot.id] ?? 0}
              busy={busy}
              onToggle={(start) => void toggle(bot, start)}
              onTest={() => void test(bot)}
              onContexts={() => setContextsBot(bot)}
              onEdit={() => startEdit(bot)}
              onDelete={() => void remove(bot)}
            />
          ))}
        </div>

        {othersByWorkspace.length > 0 ? (
          <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2.5">
            <p className="text-[12px] font-medium text-zinc-300">
              Bots em outras workspaces
            </p>
            <div className="mt-1.5 flex flex-wrap gap-1.5">
              {othersByWorkspace.map(([id, list]) => {
                const name =
                  workspaces.find((workspace) => workspace.id === id)?.name ?? id;
                const online = list.filter((bot) => bot.running).length;
                const errors = list.filter((bot) => bot.status === "error").length;
                return (
                  <Badge
                    key={id}
                    variant={errors > 0 ? "yellow" : online > 0 ? "green" : "default"}
                    title={`${online} online · ${errors} com erro`}
                  >
                    {name}: {list.length}
                  </Badge>
                );
              })}
            </div>
          </div>
        ) : null}

        {error ? (
          <p className="flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            <AlertTriangle size={12} className="mt-0.5 shrink-0" />
            {error}
          </p>
        ) : null}
      </CardContent>

      <ChannelBotContextsModal
        bot={contextsBot}
        open={contextsBot !== null}
        onClose={() => setContextsBot(null)}
        onChanged={() => void load()}
      />
    </Card>
  );
}
