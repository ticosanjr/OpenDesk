import { memo, useState, type RefObject } from "react";
import { Link } from "react-router-dom";
import {
  ArrowDown,
  ArrowUp,
  Bot,
  Loader2,
  Paperclip,
  Sparkles,
  Square,
  Volume2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/modal";
import { LazyFileViewer } from "@/components/viewer/LazyFileViewer";
import { apiGet } from "@/lib/api";
import type { ChatAttachmentMeta, FileContent } from "@/lib/types";
import type { ChatMessage, ChatThread, ChatTurn } from "@/lib/types";
import { cn } from "@/lib/utils";
import { relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import { ElapsedLabel } from "./ElapsedLabel";

function parseAttachments(metadataJson?: string | null): ChatAttachmentMeta[] {
  if (!metadataJson) return [];
  try {
    const parsed = JSON.parse(metadataJson) as { attachments?: unknown };
    if (!Array.isArray(parsed.attachments)) return [];
    return parsed.attachments.filter(
      (item): item is ChatAttachmentMeta =>
        Boolean(item) &&
        typeof item === "object" &&
        typeof (item as ChatAttachmentMeta).name === "string" &&
        typeof (item as ChatAttachmentMeta).path === "string",
    );
  } catch {
    return [];
  }
}

function byteLabel(size?: number | null): string {
  if (!size || size < 0) return "";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1_048_576).toFixed(1)} MB`;
}

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
  const [preview, setPreview] = useState<{
    attachment: ChatAttachmentMeta;
    content: FileContent | null;
    loading: boolean;
    error: string | null;
  } | null>(null);

  async function openAttachment(attachment: ChatAttachmentMeta) {
    if (!thread) return;
    setPreview({ attachment, content: null, loading: true, error: null });
    try {
      const content = await apiGet<FileContent>(
        `/api/files/content?workspace_id=${encodeURIComponent(thread.workspace_id)}&path=${encodeURIComponent(attachment.path)}`,
      );
      setPreview({ attachment, content, loading: false, error: null });
    } catch (cause) {
      setPreview({
        attachment,
        content: null,
        loading: false,
        error:
          cause instanceof Error
            ? cause.message
            : "Arquivo não encontrado no disco.",
      });
    }
  }

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
            <MessageBubble
              key={message.id}
              message={message}
              onSpeak={onSpeak}
              onOpenAttachment={(attachment) => void openAttachment(attachment)}
            />
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
      <Modal
        open={preview !== null}
        onClose={() => setPreview(null)}
        title={preview?.attachment.name ?? "Anexo"}
        size="xl"
        initialFocus="dialog"
      >
        {preview?.loading ? (
          <div className="flex items-center gap-2 text-[13px] text-zinc-500">
            <Loader2 size={13} className="animate-spin" />
            Carregando arquivo…
          </div>
        ) : preview?.error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12.5px] text-red-300">
            {preview.error}
          </p>
        ) : preview?.content && thread ? (
          <div className="h-[70vh] min-h-0">
            <LazyFileViewer
              embedded
              name={preview.attachment.name}
              path={preview.content.path}
              content={preview.content.content}
              binary={preview.content.binary}
              mime={preview.content.mime}
              truncated={preview.content.truncated}
              size={preview.content.size}
              rawUrl={`/api/files/raw?workspace_id=${encodeURIComponent(thread.workspace_id)}&path=${encodeURIComponent(preview.content.path)}`}
            />
          </div>
        ) : null}
      </Modal>
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
    onOpenAttachment,
  }: {
    message: ChatMessage;
    onSpeak: (text: string) => void;
    onOpenAttachment: (attachment: ChatAttachmentMeta) => void;
  }) {
    const attachments = parseAttachments(message.metadata_json);
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
        {attachments.length > 0 ? (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {attachments.map((attachment) => (
              <button
                key={`${attachment.direction}:${attachment.path}`}
                type="button"
                onClick={() => onOpenAttachment(attachment)}
                title={`Abrir ${attachment.path}`}
                className="inline-flex max-w-[240px] items-center gap-1.5 rounded-md border border-zinc-700/70 bg-zinc-950/60 px-2 py-1 text-left text-[11.5px] text-zinc-300 transition-colors hover:border-zinc-600 hover:text-zinc-100"
              >
                {attachment.direction === "sent" ? (
                  <ArrowUp size={11} className="shrink-0 text-blue-300" />
                ) : (
                  <ArrowDown size={11} className="shrink-0 text-emerald-300" />
                )}
                <Paperclip size={10} className="shrink-0 text-zinc-500" />
                <span className="truncate">{attachment.name}</span>
                {attachment.size ? (
                  <span className="shrink-0 font-mono text-[10.5px] text-zinc-500">
                    {byteLabel(attachment.size)}
                  </span>
                ) : null}
              </button>
            ))}
          </div>
        ) : null}
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
    previous.message.metadata_json === next.message.metadata_json &&
    previous.message.role === next.message.role,
);

export const Transcript = memo(TranscriptView);
