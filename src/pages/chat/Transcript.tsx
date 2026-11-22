import { memo, type RefObject } from "react";
import { Link } from "react-router-dom";
import { ArrowDown, Bot, Loader2, Sparkles, Square, Volume2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import type { ChatMessage, ChatThread, ChatTurn } from "@/lib/types";
import { cn } from "@/lib/utils";
import { relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import { ElapsedLabel } from "./ElapsedLabel";

function TranscriptView({
  thread,
  messages,
  thinking,
  turnRunning,
  latestTurn,
  stopping,
  transcriptRef,
  onTranscriptScroll,
  transcriptAtBottom,
  onScrollToBottom,
  onSpeak,
  onStop,
}: {
  thread: ChatThread | null;
  messages: ChatMessage[];
  thinking: boolean;
  turnRunning: boolean;
  latestTurn: ChatTurn | null;
  stopping: boolean;
  transcriptRef: RefObject<HTMLDivElement | null>;
  onTranscriptScroll: () => void;
  transcriptAtBottom: boolean;
  onScrollToBottom: () => void;
  onSpeak: (text: string) => void;
  onStop: () => void;
}) {
  return (
    <div className="relative min-h-0 flex-1 overflow-hidden">
      <div
        ref={transcriptRef}
        data-testid="chat-transcript"
        onScroll={onTranscriptScroll}
        className="h-full min-h-0 space-y-3 overflow-y-auto overscroll-contain px-4 py-3"
      >
        {!thread ? (
          <EmptyState
            icon={Bot}
            title="Escolha ou crie uma conversa"
            description="Cada conversa usa um agente e mantém o histórico no SQLite. O conteúdo é executado pelo OpenCode."
          />
        ) : messages.length === 0 ? (
          <EmptyState
            icon={Sparkles}
            title="Comece a conversa"
            description={`Envie a primeira mensagem para @${thread.agent}.`}
          />
        ) : (
          messages.map((message) => (
            <MessageBubble key={message.id} message={message} onSpeak={onSpeak} />
          ))
        )}
        {thinking ? (
          <div className="flex flex-wrap items-center gap-2 rounded-lg border border-sky-500/20 bg-sky-500/[0.05] px-3 py-2 text-[11.5px] text-sky-200/90">
            <Loader2 size={12} className="animate-spin" />
            O agente está pensando
            <ElapsedLabel
              startedAt={latestTurn?.started_at ?? null}
              running={turnRunning}
            />
            …
            {latestTurn?.run_id ? (
              <Link
                to={links.run(latestTurn.run_id)}
                className="underline decoration-sky-400/40 underline-offset-2 hover:text-sky-100"
              >
                ver execução
              </Link>
            ) : null}
            <Button
              size="xs"
              variant="outline"
              className="ml-auto border-red-500/30 text-red-200"
              onClick={onStop}
              disabled={stopping}
            >
              <Square size={10} />
              {stopping ? "parando…" : "Parar"}
            </Button>
          </div>
        ) : null}
      </div>
      {!transcriptAtBottom ? (
        <Button
          variant="outline"
          size="sm"
          className="absolute bottom-3 right-4 h-8 gap-1.5 rounded-full bg-zinc-950/95 shadow-lg"
          onClick={onScrollToBottom}
          title="Ir para a mensagem mais recente"
        >
          <ArrowDown size={13} />
          ir para o fim
        </Button>
      ) : null}
    </div>
  );
}

/**
 * Streaming chunks update the page every few hundred milliseconds. Memoizing
 * the individual bubble keeps already-rendered messages from re-rendering when
 * only the latest turn or the scroll position changes.
 */
const MessageBubble = memo(
  function MessageBubble({
    message,
    onSpeak,
  }: {
    message: ChatMessage;
    onSpeak: (text: string) => void;
  }) {
    return (
      <div
        className={cn(
          "max-w-[85%] rounded-xl px-3.5 py-2.5 text-[13.5px] leading-relaxed",
          message.role === "user"
            ? "ml-auto bg-blue-500/15 text-blue-100"
            : "border border-zinc-800/70 bg-zinc-900/50 text-zinc-300",
        )}
      >
        <p className="whitespace-pre-wrap">{message.content}</p>
        <div className="mt-1 flex items-center justify-end gap-1 font-mono text-[11px] text-zinc-500">
          {message.role === "assistant" ? (
            <button
              type="button"
              title="Ouvir resposta"
              onClick={() => onSpeak(message.content)}
              className="rounded p-0.5 text-zinc-500 hover:text-zinc-200"
            >
              <Volume2 size={11} />
            </button>
          ) : null}
          {message.role === "user" ? "você" : "agente"} ·{" "}
          {relativeTime(message.created_at)}
        </div>
      </div>
    );
  },
  (previous, next) =>
    previous.message.id === next.message.id &&
    previous.message.content === next.message.content &&
    previous.message.role === next.message.role,
);

export const Transcript = memo(TranscriptView);
