import { useEffect, useState } from "react";
import { Cpu } from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPost } from "@/lib/api";
import { pickFile } from "@/lib/folder";
import type {
  DetectResult,
  EngineStatus,
  EngineUpdateResult,
  ModelHealth,
  OpenCodeUpdateCheck,
} from "@/lib/types";
import { EngineFeedback } from "./engine/EngineFeedback";
import { EngineModelHealth } from "./engine/EngineModelHealth";
import { EngineOverview } from "./engine/EngineOverview";
import { EnginePathRow } from "./engine/EnginePathRow";
import { EngineToolbar } from "./engine/EngineToolbar";
import { EngineUpdateBox } from "./engine/EngineUpdateBox";

export function EngineCard() {
  const [engine, setEngine] = useState<EngineStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [manualPath, setManualPath] = useState("");
  const [method, setMethod] = useState("curl");
  const [check, setCheck] = useState<OpenCodeUpdateCheck | null>(null);
  const [updateOutput, setUpdateOutput] = useState<string | null>(null);
  const [diagnosticsData, setDiagnosticsData] = useState<Record<
    string,
    unknown
  > | null>(null);
  const [modelHealth, setModelHealth] = useState<ModelHealth[]>([]);

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
      if (status.resolved?.method) {
        setMethod(status.resolved.method);
      }
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  useEffect(() => {
    void load();
    void loadModelHealth();
  }, []);

  const daemon = engine?.daemon;
  const online = daemon?.online ?? false;
  const modeLabel =
    daemon?.mode === "shared"
      ? "serviço compartilhado"
      : daemon?.mode === "dedicated"
        ? "daemon dedicado"
        : "offline";

  async function loadModelHealth() {
    try {
      const value = await apiGet<{ models: ModelHealth[] }>(
        "/api/models/health",
      );
      setModelHealth(value.models);
    } catch {
      setModelHealth([]);
    }
  }

  async function clearModelHealth(provider?: string, model?: string) {
    try {
      const value = await apiPost<{ models: ModelHealth[] }>(
        "/api/models/health/clear",
        provider && model
          ? { provider_id: provider, model_id: model }
          : {},
      );
      setModelHealth(value.models);
    } catch {
      // Keep the previous list; the error is visible in diagnostics.
    }
  }

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
        method,
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

  async function rollbackEngine() {
    setBusy("rollback");
    setError(null);
    setMessage(null);
    try {
      const status = await apiPost<EngineStatus>("/api/engine/rollback");
      setEngine(status);
      setMessage("OpenCode revertido para o backup anterior.");
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
        <EngineOverview engine={engine} daemon={daemon} online={online} />

        <EnginePathRow
          manualPath={manualPath}
          onManualPathChange={setManualPath}
          busy={busy}
          onChoose={() => void chooseBinary()}
          onApply={() => void applyPath()}
        />

        <EngineToolbar
          busy={busy}
          hasDiagnostics={diagnosticsData !== null}
          onDetect={() => void detect()}
          onRestart={() => void restart()}
          onRollback={() => void rollbackEngine()}
          onDiagnostics={() => void diagnostics()}
          onRevealLogs={() => void revealLogs()}
        />

        <EngineModelHealth
          modelHealth={modelHealth}
          onClear={(provider, model) => void clearModelHealth(provider, model)}
        />

        <EngineUpdateBox
          check={check}
          daemonVersion={daemon?.version}
          method={method}
          onMethodChange={setMethod}
          busy={busy}
          onCheckUpdate={() => void checkUpdate()}
          onUpdate={() => void updateOpenCode()}
          updateOutput={updateOutput}
        />

        <EngineFeedback
          message={message}
          error={error}
          candidates={engine?.candidates}
        />
      </CardContent>
    </Card>
  );
}

function logDirFrom(value: Record<string, unknown>): string | null {
  const app = value.app as { log_dir?: string } | undefined;
  return app?.log_dir ?? null;
}
