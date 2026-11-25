import { useCallback, useEffect, useRef, useState } from "react";
import { Loader2, Mic, MicOff, Play, Volume2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPostBlob, apiPut } from "@/lib/api";
import { useVoiceInput } from "@/lib/use-voice-input";
import { toast } from "@/store/toast-store";

interface VoiceSettings {
  stt_command: string;
  stt_args: string[];
  tts_command: string;
  tts_args: string[];
  language: string;
  voice: string;
  stt_provider?: string;
  stt_endpoint?: string;
  stt_secret_ref?: string;
  tts_provider?: string;
  tts_endpoint?: string;
  tts_secret_ref?: string;
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

export function VoiceCard() {
  const [status, setStatus] = useState<VoiceStatus | null>(null);
  const [settings, setSettings] = useState<VoiceSettings | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [text, setText] = useState("Olá, esta é uma prova de voz do OpenDesk.");
  const [audio, setAudio] = useState<string | null>(null);
  const [transcript, setTranscript] = useState("");
  const audioUrl = useRef<string | null>(null);

  const voice = useVoiceInput({
    onTranscript: (result) => {
      setTranscript(result.text);
      toast({
        tone: "success",
        title: `Transcrito com ${result.provider ?? "STT"}`,
        description: result.text.slice(0, 120),
      });
    },
    onError: (message) =>
      toast({ tone: "error", title: "Transcrição falhou", description: message }),
  });

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
      const blob = await apiPostBlob("/api/voice/tts", { text });
      if (audioUrl.current) URL.revokeObjectURL(audioUrl.current);
      const url = URL.createObjectURL(blob);
      audioUrl.current = url;
      setAudio(url);
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

  useEffect(
    () => () => {
      if (audioUrl.current) URL.revokeObjectURL(audioUrl.current);
    },
    [],
  );

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
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  Provedores STT (ordem)
                </span>
                <input
                  value={settings.stt_provider ?? "auto"}
                  onChange={(event) =>
                    setSettings({ ...settings, stt_provider: event.target.value })
                  }
                  placeholder="auto, whisper, openai"
                  className={inputClass}
                />
              </label>
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  Endpoint STT (opcional)
                </span>
                <input
                  value={settings.stt_endpoint ?? ""}
                  onChange={(event) =>
                    setSettings({ ...settings, stt_endpoint: event.target.value })
                  }
                  placeholder="https://api.openai.com"
                  className={inputClass}
                />
              </label>
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  secret_ref STT
                </span>
                <input
                  value={settings.stt_secret_ref ?? ""}
                  onChange={(event) =>
                    setSettings({ ...settings, stt_secret_ref: event.target.value })
                  }
                  placeholder="openai"
                  className={inputClass}
                />
              </label>
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  Provedores TTS (ordem)
                </span>
                <input
                  value={settings.tts_provider ?? "auto"}
                  onChange={(event) =>
                    setSettings({ ...settings, tts_provider: event.target.value })
                  }
                  placeholder="piper, espeak, openai"
                  className={inputClass}
                />
              </label>
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  Endpoint TTS (opcional)
                </span>
                <input
                  value={settings.tts_endpoint ?? ""}
                  onChange={(event) =>
                    setSettings({ ...settings, tts_endpoint: event.target.value })
                  }
                  placeholder="https://api.openai.com"
                  className={inputClass}
                />
              </label>
              <label>
                <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
                  secret_ref TTS
                </span>
                <input
                  value={settings.tts_secret_ref ?? ""}
                  onChange={(event) =>
                    setSettings({ ...settings, tts_secret_ref: event.target.value })
                  }
                  placeholder="openai"
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
                Testar fala (TTS)
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
                  variant={voice.recording ? "outline" : "default"}
                  size="sm"
                  className="h-7"
                  disabled={voice.transcribing}
                  onClick={() =>
                    voice.recording ? voice.stop() : voice.start()
                  }
                >
                  {voice.recording ? <MicOff size={11} /> : <Mic size={11} />}
                  {voice.recording
                    ? `Parar (${voice.seconds}s)`
                    : voice.transcribing
                      ? "Transcrevendo…"
                      : "Testar transcrição (STT)"}
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
