import { useCallback, useEffect, useRef, useState } from "react";
import { apiPost } from "@/lib/api";

export interface VoiceTranscript {
  text: string;
  provider?: string;
  language?: string;
  duration_ms?: number;
}

export function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const value = String(reader.result ?? "");
      const comma = value.indexOf(",");
      resolve(comma >= 0 ? value.slice(comma + 1) : value);
    };
    reader.onerror = () => reject(reader.error ?? new Error("falha ao ler áudio"));
    reader.readAsDataURL(blob);
  });
}

/** Appends text without creating double spaces. */
export function appendTranscript(current: string, text: string): string {
  const clean = text.trim();
  if (!clean) return current;
  return current.trim() ? `${current.trimEnd()} ${clean}` : clean;
}

/** Inserts at the caret when the field is available, otherwise appends. */
export function insertAtCursor(
  element: HTMLTextAreaElement | HTMLInputElement | null,
  current: string,
  text: string,
): string {
  const clean = text.trim();
  if (!element || !clean) return appendTranscript(current, clean);
  const start = element.selectionStart ?? current.length;
  const end = element.selectionEnd ?? start;
  return `${current.slice(0, start)}${clean}${current.slice(end)}`;
}

export function useVoiceInput({
  onTranscript,
  onError,
}: {
  onTranscript: (transcript: VoiceTranscript) => void;
  onError?: (message: string) => void;
}) {
  const [recording, setRecording] = useState(false);
  const [transcribing, setTranscribing] = useState(false);
  const [seconds, setSeconds] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const recorder = useRef<MediaRecorder | null>(null);
  const stream = useRef<MediaStream | null>(null);
  const chunks = useRef<Blob[]>([]);
  const cancelled = useRef(false);
  const timer = useRef<number | null>(null);

  const release = useCallback(() => {
    if (timer.current !== null) {
      window.clearInterval(timer.current);
      timer.current = null;
    }
    stream.current?.getTracks().forEach((track) => track.stop());
    stream.current = null;
  }, []);

  const stop = useCallback(() => {
    recorder.current?.stop();
    recorder.current = null;
    setRecording(false);
    if (timer.current !== null) {
      window.clearInterval(timer.current);
      timer.current = null;
    }
  }, []);

  const cancel = useCallback(() => {
    cancelled.current = true;
    stop();
    release();
    setSeconds(0);
  }, [release, stop]);

  const fail = useCallback(
    (message: string) => {
      setError(message);
      onError?.(message);
    },
    [onError],
  );

  const transcribe = useCallback(
    async (blob: Blob) => {
      setTranscribing(true);
      setError(null);
      try {
        const audio_base64 = await blobToBase64(blob);
        const result = await apiPost<VoiceTranscript>("/api/voice/stt", {
          audio_base64,
          mime: blob.type || "audio/webm",
        });
        if (result.text.trim()) onTranscript(result);
      } catch (cause) {
        fail(cause instanceof Error ? cause.message : String(cause));
      } finally {
        setTranscribing(false);
      }
    },
    [fail, onTranscript],
  );

  const start = useCallback(async () => {
    if (recording || transcribing) return;
    setError(null);
    setSeconds(0);
    cancelled.current = false;
    try {
      const media = await navigator.mediaDevices.getUserMedia({ audio: true });
      stream.current = media;
      const recorderInstance = new MediaRecorder(
        media,
        MediaRecorder.isTypeSupported("audio/webm;codecs=opus")
          ? { mimeType: "audio/webm;codecs=opus" }
          : undefined,
      );
      chunks.current = [];
      recorderInstance.ondataavailable = (event) => {
        if (event.data.size > 0) chunks.current.push(event.data);
      };
      recorderInstance.onstop = () => {
        const wasCancelled = cancelled.current;
        release();
        setSeconds(0);
        if (wasCancelled) return;
        const blob = new Blob(chunks.current, {
          type: recorderInstance.mimeType || "audio/webm",
        });
        void transcribe(blob);
      };
      recorderInstance.start();
      recorder.current = recorderInstance;
      setRecording(true);
      timer.current = window.setInterval(
        () => setSeconds((value) => value + 1),
        1000,
      );
    } catch (cause) {
      release();
      const name = cause instanceof DOMException ? cause.name : "";
      fail(
        name === "NotAllowedError" || name === "SecurityError"
          ? "Permissão de microfone negada. Autorize o microfone para ditar."
          : cause instanceof Error
            ? cause.message
            : "Microfone indisponível.",
      );
    }
  }, [fail, recording, release, transcribe, transcribing]);

  useEffect(
    () => () => {
      cancelled.current = true;
      recorder.current?.stop();
      recorder.current = null;
      release();
    },
    [release],
  );

  return {
    recording,
    transcribing,
    seconds,
    error,
    start: () => void start(),
    stop,
    cancel,
    clearError: () => setError(null),
  };
}
