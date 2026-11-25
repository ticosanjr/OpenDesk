import { useCallback, useEffect, useRef, useState } from "react";
import { apiPostBlob } from "@/lib/api";

export function useVoiceOutput() {
  const [speaking, setSpeaking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const urlRef = useRef<string | null>(null);

  const cleanup = useCallback(() => {
    audioRef.current?.pause();
    audioRef.current = null;
    if (urlRef.current) {
      URL.revokeObjectURL(urlRef.current);
      urlRef.current = null;
    }
    setSpeaking(false);
  }, []);

  const stop = useCallback(() => {
    cleanup();
  }, [cleanup]);

  const play = useCallback(
    async (text: string, workspaceId?: string | null) => {
      const value = text.trim();
      if (!value) return;
      cleanup();
      setError(null);
      setSpeaking(true);
      try {
        const blob = await apiPostBlob("/api/voice/tts", {
          text: value,
          workspace_id: workspaceId ?? undefined,
        });
        const url = URL.createObjectURL(blob);
        urlRef.current = url;
        const audio = new Audio(url);
        audioRef.current = audio;
        audio.onended = cleanup;
        audio.onerror = () => {
          setError("O áudio não pôde ser reproduzido.");
          cleanup();
        };
        await audio.play();
      } catch (cause) {
        setSpeaking(false);
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [cleanup],
  );

  useEffect(() => cleanup, [cleanup]);

  return { speaking, error, play, stop };
}
