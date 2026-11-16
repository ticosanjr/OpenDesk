import { useCallback, useEffect, useRef, useState } from "react";
import { Loader2, Mic, MicOff, Play, Volume2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";

interface VoiceSettings {
  stt_command: string;
  stt_args: string[];
  tts_command: string;
  tts_args: string[];
  language: string;
  voice: string;
}

interface VoiceStatus {
  settings: VoiceSettings;
  capabilities: {
    stt?: string | null;
    tts?: string | null;
    available: boolean;
    reason?: string | null;
  };
}

const inputClass =
  "h-8 w-full rounded border border-zinc-700 bg-zinc-950 px-2 text-[12.5px] text-zinc-200";

function base64FromBlob(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result ?? "");
      resolve(result.includes(",") ? result.split(",")[1] : result);
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

export function VoiceCard() {
  const [status, setStatus] = useState<VoiceStatus | null>(null);
  const [settings, setSettings] = useState<VoiceSettings | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [text, setText] = useState("Olá, esta é uma prova de voz do OpenDesk.");
  const [audio, setAudio] = useState<string | null>(null);
  const [transcript, setTranscript] = useState("");
  const [recording, setRecording] = useState(false);
  const recorder = useRef<MediaRecorder | null>(null);
  const chunks = useRef<Blob[]>([]);

  const load = useCallback(async () => {
    try {
      const value = await apiGet<VoiceStatus>("/api/voice/status");
      setStatus(value);
      setSettings(value.settings);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar a voz",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function save() {
    if (!settings) return;
    setBusy("save");
    try {
      const value = await apiPut<VoiceStatus>("/api/voice/settings", settings);
      setStatus(value);
      setSettings(value.settings);
      toast({ tone: "success", title: "Configuração de voz salva" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao salvar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function speak() {
    if (!text.trim()) return;
    setBusy("speak");
    try {
      const result = await apiPost<{ data: string; mime: string; path: string }>(
        "/api/voice/speak",
        { text },
      );
      setAudio(`data:${result.mime};base64,${result.data}`);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Fala indisponível",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function startRecording() {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const media = new MediaRecorder(stream);
      chunks.current = [];
      media.ondataavailable = (event) => {
        if (event.data.size > 0) chunks.current.push(event.data);
      };
      media.onstop = () => {
        stream.getTracks().forEach((track) => track.stop());
        void transcribe(new Blob(chunks.current, { type: media.mimeType || "audio/webm" }));
      };
      media.start();
      recorder.current = media;
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
    recorder.current?.stop();
    recorder.current = null;
    setRecording(false);
  }

  async function transcribe(blob: Blob) {
    setBusy("transcribe");
    try {
      const audio_base64 = await base64FromBlob(blob);
      const result = await apiPost<{ text: string; engine: string }>(
        "/api/voice/transcribe",
        { audio_base64, mime: blob.type || "audio/webm" },
      );
      setTranscript(result.text);
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
      setBusy(null);
    }
  }

  return (
    <Card id="settings-voice" className="scroll-mt-14">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Volume2 size={15} className="text-zinc-500" />
            Voz (STT/TTS)
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Usa motores locais do host. Modelos não são baixados pelo OpenDesk.
            Placeholders: <code className="font-mono">{"{audio}"}</code>,{" "}
            <code className="font-mono">{"{out_base}"}</code>,{" "}
            <code className="font-mono">{"{text}"}</code>.
          </p>
        </div>
        {status ? (
          <Badge variant={status.capabilities.available ? "green" : "red"}>
            {status.capabilities.available
              ? `${status.capabilities.stt} → ${status.capabilities.tts}`
              : "indisponível"}
          </Badge>
        ) : null}
      </CardHeader>
      <CardContent className="space-y-3">
        {status && !status.capabilities.available ? (
          <p className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-[12px] text-amber-200">
            {status.capabilities.reason}
          </p>
        ) : null}
        {settings ? (
          <>
            <div className="grid gap-2 sm:grid-cols-2">
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  Comando STT
                </span>
                <input
                  value={settings.stt_command}
                  onChange={(event) =>
                    setSettings({ ...settings, stt_command: event.target.value })
                  }
                  placeholder="whisper-cli"
                  className={inputClass}
                />
              </label>
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  Comando TTS
                </span>
                <input
                  value={settings.tts_command}
                  onChange={(event) =>
                    setSettings({ ...settings, tts_command: event.target.value })
                  }
                  placeholder="piper, espeak-ng, say…"
                  className={inputClass}
                />
              </label>
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  Idioma
                </span>
                <input
                  value={settings.language}
                  onChange={(event) =>
                    setSettings({ ...settings, language: event.target.value })
                  }
                  className={inputClass}
                />
              </label>
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  Voz (opcional)
                </span>
                <input
                  value={settings.voice}
                  onChange={(event) =>
                    setSettings({ ...settings, voice: event.target.value })
                  }
                  className={inputClass}
                />
              </label>
            </div>
            <div className="flex justify-end">
              <Button
                size="sm"
                className="h-7"
                disabled={busy === "save"}
                onClick={() => void save()}
              >
                Salvar
              </Button>
            </div>

            <div className="grid gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 sm:grid-cols-[minmax(0,1fr)_auto]">
              <textarea
                value={text}
                onChange={(event) => setText(event.target.value)}
                rows={2}
                placeholder="texto para falar"
                className="w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-[12.5px] text-zinc-200"
              />
              <Button
                size="sm"
                className="h-8 self-end"
                disabled={busy === "speak"}
                onClick={() => void speak()}
              >
                {busy === "speak" ? (
                  <Loader2 size={12} className="animate-spin" />
                ) : (
                  <Play size={12} />
                )}
                Falar
              </Button>
              {audio ? (
                <audio controls src={audio} className="sm:col-span-2 w-full" />
              ) : null}
            </div>

            <div className="space-y-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
              <div className="flex items-center justify-between gap-2">
                <span className="text-[12.5px] font-medium text-zinc-300">
                  Transcrever do microfone
                </span>
                <Button
                  variant={recording ? "outline" : "default"}
                  size="sm"
                  className="h-7"
                  disabled={busy === "transcribe"}
                  onClick={() => (recording ? stopRecording() : void startRecording())}
                >
                  {recording ? <MicOff size={11} /> : <Mic size={11} />}
                  {recording ? "Parar" : "Gravar"}
                </Button>
              </div>
              <p className="text-[11.5px] text-zinc-500">
                Gravação fica no navegador; só o áudio é enviado ao motor local.
              </p>
              {transcript ? (
                <textarea
                  value={transcript}
                  onChange={(event) => setTranscript(event.target.value)}
                  rows={3}
                  className="w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-[12.5px] text-zinc-200"
                />
              ) : null}
            </div>
          </>
        ) : (
          <Loader2 size={14} className="animate-spin text-zinc-500" />
        )}
      </CardContent>
    </Card>
  );
}
