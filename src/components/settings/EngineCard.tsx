import { useEffect, useState } from "react";
import {
  AlertTriangle,
  Check,
  Clipboard,
  Cpu,
  Download,
  FileSearch,
  Loader2,
  RefreshCw,
  Search,
  Terminal,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPost } from "@/lib/api";
import { pickFile } from "@/lib/folder";
import { shortPath } from "@/lib/format";
import type {
  DetectResult,
  EngineStatus,
  EngineUpdateResult,
  OpenCodeUpdateCheck,
} from "@/lib/types";

export function EngineCard() {
  const [engine, setEngine] = useState<EngineStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [manualPath, setManualPath] = useState("");
  const [check, setCheck] = useState<OpenCodeUpdateCheck | null>(null);
  const [updateOutput, setUpdateOutput] = useState<string | null>(null);
  const [diagnosticsData, setDiagnosticsData] = useState<Record<
    string,
    unknown
  > | null>(null);

  async function load() {
    try {
      const status = await apiGet<EngineStatus>("/api/engine/status");
      setEngine(status);
      setManualPath(
        (current) =>
          current ||
          status.resolved?.path ||
          status.configured_binary ||
          "",
      );
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  useEffect(() => {
    void load();
  }, []);

  const daemon = engine?.daemon;
  const online = daemon?.online ?? false;
  const modeLabel =
    daemon?.mode === "shared"
      ? "serviço compartilhado"
      : daemon?.mode === "dedicated"
        ? "daemon dedicado"
        : "offline";

  async function detect() {
    setBusy("detect");
    setError(null);
    setMessage(null);
    try {
      const result = await apiPost<DetectResult>("/api/engine/detect");
      if (result.ok && result.binary) {
        setManualPath(result.binary.path);
        setMessage(
          `Encontrado ${result.binary.path} (v${result.binary.version}).`,
        );
      } else {
        setError(result.error ?? "OpenCode V2 não encontrado.");
      }
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function chooseBinary() {
    const selected = await pickFile(
      "Selecione o binário do OpenCode",
      manualPath || undefined,
    );
    if (selected) {
      setManualPath(selected);
      setMessage("Caminho selecionado. Clique em Aplicar caminho.");
    }
  }

  async function applyPath() {
    if (!manualPath.trim()) {
      setError("Informe o caminho completo do binário opencode.");
      return;
    }
    setBusy("apply");
    setError(null);
    setMessage(null);
    try {
      const status = await apiPost<EngineStatus>("/api/engine/binary", {
        path: manualPath.trim(),
      });
      setEngine(status);
      setMessage(
        status.daemon.online
          ? `Motor online usando ${status.resolved?.path ?? "o caminho salvo"}.`
          : "Caminho salvo, mas o motor ainda não subiu. Veja a mensagem abaixo.",
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function restart() {
    setBusy("restart");
    setError(null);
    setMessage(null);
    try {
      const status = await apiPost<EngineStatus>("/api/engine/restart");
      setEngine(status);
      setMessage(
        status.daemon.online
          ? "Motor reiniciado com sucesso."
          : "O motor não subiu. Veja a mensagem e os caminhos testados.",
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function checkUpdate() {
    setBusy("check");
    setError(null);
    setMessage(null);
    try {
      const result = await apiPost<OpenCodeUpdateCheck>("/api/engine/update/check");
      setCheck(result);
      if (result.error) {
        setError(result.error);
      } else if (result.update_available) {
        setMessage(`Nova versão do OpenCode disponível: v${result.latest}.`);
      } else {
        setMessage("Você já está na última versão estável do OpenCode V2.");
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function updateOpenCode() {
    if (!check?.latest) {
      return;
    }
    setBusy("update");
    setError(null);
    setMessage(null);
    setUpdateOutput(null);
    try {
      const result = await apiPost<EngineUpdateResult>("/api/engine/update", {
        version: check.latest,
      });
      setUpdateOutput(result.output.trim());
      if (result.ok) {
        setMessage(`OpenCode atualizado para v${result.version ?? check.latest}.`);
      } else {
        setError(
          "A atualização não foi confirmada. O motor continua na versão anterior ou precisa de reinício manual.",
        );
      }
      await load();
      await checkUpdate();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function diagnostics() {
    setBusy("diagnostics");
    setError(null);
    try {
      const value = await apiGet<Record<string, unknown>>(
        "/api/engine/diagnostics",
      );
      const text = JSON.stringify(value, null, 2);
      setDiagnosticsData(value);
      try {
        await navigator.clipboard.writeText(text);
        setMessage("Diagnóstico copiado para a área de transferência.");
      } catch {
        setUpdateOutput(text);
        setMessage("Diagnóstico gerado abaixo (copie manualmente).");
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function revealLogs() {
    const path = diagnosticsData ? logDirFrom(diagnosticsData) : null;
    if (path) {
      try {
        await openPath(path);
      } catch {
        // Ignore; the diagnostics output already contains the path.
      }
    }
  }

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Cpu size={15} className="text-zinc-500" />
            OpenCode Engine
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            O OpenDesk usa OpenCode V2. Se o binário não estiver no PATH do
            AppImage, informe o caminho aqui — ele fica salvo e é usado nos
            próximos inícios.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant={online ? "green" : "red"}>
            {online ? "online" : "offline"}
          </Badge>
          <Badge variant="default">{modeLabel}</Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="grid gap-2 md:grid-cols-4">
          <Info label="Versão" value={daemon?.version ?? "—"} />
          <Info label="Porta" value={daemon?.port ? String(daemon.port) : "—"} />
          <Info
            label="Binário"
            value={
              daemon?.binary_path
                ? shortPath(daemon.binary_path, 48)
                : engine?.resolved?.path
                  ? shortPath(engine.resolved.path, 48)
                  : "não detectado"
            }
          />
          <Info
            label="Método"
            value={engine?.resolved?.method ?? "desconhecido"}
          />
        </div>

        {!online && (daemon?.message || engine?.resolve_error) ? (
          <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] leading-relaxed text-amber-200">
            {daemon?.message ?? engine?.resolve_error}
          </p>
        ) : null}

        <div className="flex flex-col gap-2 md:flex-row">
          <input
            value={manualPath}
            onChange={(event) => setManualPath(event.target.value)}
            placeholder="/home/usuario/.opencode/bin/opencode"
            className="h-9 min-w-0 flex-1 rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 font-mono text-[11px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60"
          />
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              className="h-9"
              onClick={() => void chooseBinary()}
            >
              <FileSearch size={13} />
              Escolher
            </Button>
            <Button
              size="sm"
              className="h-9"
              onClick={() => void applyPath()}
              disabled={busy !== null}
            >
              {busy === "apply" ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Check size={13} />
              )}
              Aplicar caminho
            </Button>
          </div>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void detect()}
            disabled={busy !== null}
          >
            {busy === "detect" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Search size={13} />
            )}
            Detectar novamente
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void restart()}
            disabled={busy !== null}
          >
            {busy === "restart" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <RefreshCw size={13} />
            )}
            Reiniciar motor
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void diagnostics()}
            disabled={busy !== null}
          >
            {busy === "diagnostics" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Clipboard size={13} />
            )}
            Copiar diagnóstico
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void revealLogs()}
            disabled={diagnosticsData === null}
          >
            <Terminal size={13} />
            Abrir logs
          </Button>
        </div>

        <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
          <div className="flex flex-wrap items-center gap-2">
            <p className="flex-1 text-[11px] text-zinc-300">
              OpenCode instalado:{" "}
              <span className="font-mono text-zinc-100">
                {check?.current ?? daemon?.version ?? "desconhecido"}
              </span>
              {check?.latest ? (
                <>
                  {" "}
                  · última V2:{" "}
                  <span className="font-mono text-zinc-100">
                    {check.latest}
                  </span>
                </>
              ) : null}
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void checkUpdate()}
              disabled={busy !== null}
            >
              {busy === "check" ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Search size={13} />
              )}
              Verificar atualização
            </Button>
            {check?.update_available ? (
              <Button
                size="sm"
                onClick={() => void updateOpenCode()}
                disabled={busy !== null}
              >
                {busy === "update" ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Download size={13} />
                )}
                Atualizar OpenCode
              </Button>
            ) : null}
          </div>
          {check?.release?.notes ? (
            <p className="mt-2 max-h-24 overflow-y-auto whitespace-pre-wrap text-[9px] leading-relaxed text-zinc-500">
              {check.release.notes}
            </p>
          ) : null}
          {updateOutput ? (
            <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap rounded bg-black/40 p-2 font-mono text-[9px] text-zinc-400">
              {updateOutput}
            </pre>
          ) : null}
        </div>

        {message ? (
          <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[10px] leading-relaxed text-emerald-300">
            {message}
          </p>
        ) : null}
        {error ? (
          <p className="flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] leading-relaxed text-red-300">
            <AlertTriangle size={12} className="mt-0.5 shrink-0" />
            {error}
          </p>
        ) : null}

        {engine && engine.candidates.length > 0 ? (
          <details className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[9px] text-zinc-500">
            <summary className="cursor-pointer text-zinc-400">
              Caminhos testados ({engine.candidates.length})
            </summary>
            <ul className="mt-2 space-y-0.5 font-mono">
              {engine.candidates.slice(0, 24).map((candidate) => (
                <li key={candidate} className="truncate">
                  {candidate}
                </li>
              ))}
            </ul>
          </details>
        ) : null}
      </CardContent>
    </Card>
  );
}

function Info({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
      <p className="text-[9px] uppercase tracking-wide text-zinc-600">{label}</p>
      <p className="mt-0.5 truncate font-mono text-[11px] text-zinc-200">
        {value}
      </p>
    </div>
  );
}

function logDirFrom(value: Record<string, unknown>): string | null {
  const app = value.app as { log_dir?: string } | undefined;
  return app?.log_dir ?? null;
}
