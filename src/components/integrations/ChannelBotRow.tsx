import {
  AlertTriangle,
  Loader2,
  MessageSquare,
  Pencil,
  Play,
  Send,
  Square,
  Trash2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { StatusDot, type DotTone } from "@/components/ui/status-dot";
import type { ChannelBot } from "@/lib/types";

function statusOf(bot: ChannelBot): { label: string; tone: DotTone } {
  if (bot.running) return { label: "online", tone: "online" };
  if (bot.status === "error") return { label: "erro", tone: "warning" };
  if (bot.enabled) return { label: "parado", tone: "offline" };
  return { label: "desabilitado", tone: "offline" };
}

export function ChannelBotRow({
  bot,
  count,
  busy,
  onToggle,
  onTest,
  onContexts,
  onEdit,
  onDelete,
}: {
  bot: ChannelBot;
  count: number;
  busy: string | null;
  onToggle: (start: boolean) => void;
  onTest: () => void;
  onContexts: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const status = statusOf(bot);
  const starting = busy === `start:${bot.id}`;
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
      <div className="flex flex-wrap items-center gap-2">
        <StatusDot tone={starting ? "starting" : status.tone} />
        <span className="min-w-0 truncate text-[13.5px] font-medium text-zinc-200">
          {bot.name}
        </span>
        <Badge
          variant={
            bot.running ? "green" : bot.status === "error" ? "yellow" : "default"
          }
        >
          {status.label}
        </Badge>
        <Badge variant={bot.token_configured ? "blue" : "red"}>
          token {bot.token_configured ? "ok" : "ausente"}
        </Badge>
        <span className="text-[11.5px] text-zinc-500">
          agente {bot.default_agent}
        </span>
      </div>
      {bot.last_error ? (
        <p className="mt-1 flex items-start gap-1.5 text-[11.5px] text-amber-300">
          <AlertTriangle size={11} className="mt-0.5 shrink-0" />
          {bot.last_error}
        </p>
      ) : null}
      <div className="mt-2 flex flex-wrap gap-1.5">
        {bot.running ? (
          <Button
            variant="outline"
            size="sm"
            onClick={() => onToggle(false)}
            disabled={busy !== null}
          >
            <Square size={11} />
            Parar
          </Button>
        ) : (
          <Button
            variant="outline"
            size="sm"
            onClick={() => onToggle(true)}
            disabled={busy !== null}
          >
            {starting ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <Play size={11} />
            )}
            Iniciar
          </Button>
        )}
        <Button
          variant="outline"
          size="sm"
          onClick={onTest}
          disabled={busy !== null || !bot.token_configured}
        >
          {busy === `test:${bot.id}` ? (
            <Loader2 size={12} className="animate-spin" />
          ) : (
            <Send size={11} />
          )}
          Testar
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={onContexts}
          disabled={busy !== null}
        >
          <MessageSquare size={11} />
          Contextos ({count})
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={onEdit}
          disabled={busy !== null}
        >
          <Pencil size={11} />
          Editar
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={onDelete}
          disabled={busy !== null}
        >
          {busy === `delete:${bot.id}` ? (
            <Loader2 size={11} className="animate-spin" />
          ) : (
            <Trash2 size={11} />
          )}
          Excluir
        </Button>
      </div>
    </div>
  );
}
