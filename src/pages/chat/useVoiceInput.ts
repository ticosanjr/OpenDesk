import { useCallback, useRef, useState } from "react";
import { apiPost } from "@/lib/api";
import { toast } from "@/store/toast-store";
import { base64FromBlob } from "./helpers";

export function useVoiceInput({
  workspaceId,
  setPrompt,
}: {
  workspaceId: string | null;
  setPrompt: (update: (current: string) => string) => void;
}) {
  const [recording, setRecording] = useState(false);
  const [voiceBusy, setVoiceBusy] = useState(false);
  const recorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);

  const transcribeVoice = useCallback(
    async (blob: Blob) => {
      setVoiceBusy(true);
      try {
        const audio_base64 = await base64FromBlob(blob);
        const result = await apiPost<{ text: string; engine: string }>(
          "/api/voice/transcribe",
          { audio_base64, mime: blob.type || "audio/webm" },
        );
        setPrompt((current) => (current ? `${current} ${result.text}` : result.text));
        toast({
          tone: "success",
          title: `Transcrito com ${result.engine}`,
          description: result.text.slice(0, 120),
        });
      } catch (cause) {
        toast({
          tone: "error",
          title: "Transcrição falhou",
          description: cause instanceof Error ? cause.message : String(cause),
        });
      } finally {
        setVoiceBusy(false);
      }
    },
    [setPrompt],
  );

  async function startRecording() {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const media = new MediaRecorder(stream);
      chunksRef.current = [];
      media.ondataavailable = (event) => {
        if (event.data.size > 0) chunksRef.current.push(event.data);
      };
      media.onstop = () => {
        stream.getTracks().forEach((track) => track.stop());
        void transcribeVoice(
          new Blob(chunksRef.current, { type: media.mimeType || "audio/webm" }),
        );
      };
      media.start();
      recorderRef.current = media;
      setRecording(true);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Microfone indisponível",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  function stopRecording() {
    recorderRef.current?.stop();
    recorderRef.current = null;
    setRecording(false);
  }

  async function speakMessage(text: string) {
    if (!text.trim()) return;
    setVoiceBusy(true);
    try {
      const result = await apiPost<{ data: string; mime: string }>(
        "/api/voice/speak",
        { text, workspace_id: workspaceId },
      );
      const audio = new Audio(`data:${result.mime};base64,${result.data}`);
      await audio.play();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Fala indisponível",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setVoiceBusy(false);
    }
  }

  return { recording, voiceBusy, startRecording, stopRecording, speakMessage };
}
