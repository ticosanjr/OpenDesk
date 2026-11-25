import { Loader2, Paperclip, Send, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { VoiceDictationButton } from "@/components/ui/voice-dictation-button";
import { appendTranscript } from "@/lib/use-voice-input";
import type { ChatThread } from "@/lib/types";

export interface ChatAttachment {
  name: string;
  data: string;
}

export function ChatComposer({
  thread,
  workspaceId,
  prompt,
  onPromptChange,
  attachments,
  onFiles,
  onRemoveAttachment,
  sending,
  onSend,
}: {
  thread: ChatThread | null;
  workspaceId: string;
  prompt: string;
  onPromptChange: (value: string) => void;
  attachments: ChatAttachment[];
  onFiles: (files: File[]) => void;
  onRemoveAttachment: (index: number) => void;
  sending: boolean;
  onSend: () => void;
}) {
  return (
    <div
      data-testid="chat-composer"
      className="shrink-0 border-t border-zinc-800/70 p-3"
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault();
        onFiles(Array.from(event.dataTransfer.files));
      }}
      onPaste={(event) => {
        const files = Array.from(event.clipboardData.files);
        if (files.length > 0) {
          event.preventDefault();
          onFiles(files);
        }
      }}
    >
      {attachments.length > 0 ? (
        <div className="mb-2 flex flex-wrap gap-1.5">
          {attachments.map((file, index) => (
            <span
              key={`${file.name}-${index}`}
              className="inline-flex items-center gap-1 rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1 text-[11.5px] text-zinc-400"
            >
              <Paperclip size={10} />
              <span className="max-w-40 truncate">{file.name}</span>
              <button
                type="button"
                onClick={() => onRemoveAttachment(index)}
                className="text-zinc-500 hover:text-zinc-200"
              >
                <X size={10} />
              </button>
            </span>
          ))}
        </div>
      ) : null}
      <div className="flex w-full items-stretch gap-2">
        <SlashTextarea
          containerClassName="min-w-0 flex-1"
          value={prompt}
          onChange={onPromptChange}
          workspaceId={workspaceId}
          rows={2}
          submitOnEnter
          onSubmit={onSend}
          disabled={!thread || sending}
          placeholder={
            thread
              ? `Mensagem para @${thread.agent}… (Enter envia, Shift+Enter quebra linha, cole/arraste arquivos)`
              : "Crie ou selecione uma conversa primeiro"
          }
          className="min-h-[52px] flex-1"
        />
        <VoiceDictationButton
          disabled={!thread || sending}
          className="h-auto min-h-[52px] w-12"
          onTranscript={(transcript) =>
            onPromptChange(appendTranscript(prompt, transcript.text))
          }
        />
        <Button
          size="sm"
          className="h-auto min-h-[52px] w-24 shrink-0"
          onClick={onSend}
          disabled={!thread || sending || (!prompt.trim() && attachments.length === 0)}
        >
          {sending ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <Send size={14} />
          )}
          Enviar
        </Button>
      </div>
    </div>
  );
}
