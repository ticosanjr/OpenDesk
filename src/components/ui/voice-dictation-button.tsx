import { Loader2, Mic, Square } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useVoiceInput, type VoiceTranscript } from "@/lib/use-voice-input";
import { cn } from "@/lib/utils";
import { toast } from "@/store/toast-store";

export function VoiceDictationButton({
  onTranscript,
  onError,
  disabled = false,
  className,
  label = "Ditar por voz",
}: {
  onTranscript: (transcript: VoiceTranscript) => void;
  onError?: (message: string) => void;
  disabled?: boolean;
  className?: string;
  label?: string;
}) {
  const voice = useVoiceInput({
    onTranscript,
    onError: (message) => {
      if (onError) onError(message);
      else toast({ tone: "error", title: "Microfone", description: message });
    },
  });
  const title = voice.error
    ? voice.error
    : voice.recording
      ? `Parar gravação (${voice.seconds}s)`
      : voice.transcribing
        ? "Transcrevendo…"
        : label;

  return (
    <Button
      type="button"
      variant={voice.recording ? "destructive" : "ghost"}
      size="icon"
      className={cn(
        "h-8 w-8 shrink-0",
        voice.recording && "animate-pulse",
        className,
      )}
      disabled={disabled || voice.transcribing}
      aria-label={voice.recording ? "Parar gravação" : label}
      aria-pressed={voice.recording}
      title={title}
      onClick={() => (voice.recording ? voice.stop() : voice.start())}
    >
      {voice.transcribing ? (
        <Loader2 size={13} className="animate-spin" />
      ) : voice.recording ? (
        <Square size={12} fill="currentColor" />
      ) : (
        <Mic size={13} />
      )}
    </Button>
  );
}
