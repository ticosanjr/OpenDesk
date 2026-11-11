import { useCallback, useEffect, useState } from "react";
import QRCode from "qrcode";
import {
  AlertTriangle,
  Check,
  Link2,
  Loader2,
  MessageCircle,
  Play,
  Save,
  Square,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { useAppStore } from "@/store/app-store";
import type { SignalStatus } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  smallInputClass as inputClass,
  smallLabelClass as labelClass,
} from "@/lib/ui-classes";

export function SignalCard() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [status, setStatus] = useState<SignalStatus | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [account, setAccount] = useState("");
  const [cliPath, setCliPath] = useState("signal-cli");
  const [allowlist, setAllowlist] = useState("");
  const [workspaceId, setWorkspaceId] = useState("");
  const [agent, setAgent] = useState("build");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [linkUri, setLinkUri] = useState<string | null>(null);
  const [qr, setQr] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const value = await apiGet<SignalStatus>("/api/integrations/signal");
      setStatus(value);
      setEnabled(value.config.enabled);
      setAccount(value.config.account);
      setCliPath(value.config.cli_path || "signal-cli");
      setAllowlist(value.config.allowlist.join(", "));
      setWorkspaceId(value.config.workspace_id ?? activeWorkspaceId ?? "");
      setAgent(value.config.agent || "build");
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [activeWorkspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function save() {
    setBusy("save");
    setError(null);
    setMessage(null);
    try {
      const value = await apiPut<SignalStatus>("/api/integrations/signal", {
        enabled,
        account: account.trim(),
        cli_path: cliPath.trim(),
        allowlist: allowlist
          .split(/[,\n]/)
          .map((item) => item.trim())
          .filter(Boolean),
        workspace_id: workspaceId || null,
        agent: agent.trim() || "build",
      });
      setStatus(value);
      setMessage(
        value.bridge_running
          ? "Configuração salva e bridge do Signal em execução."
          : "Configuração salva. O bridge inicia quando habilitado e vinculado.",
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function link() {
    setBusy("link");
    setError(null);
    setMessage(null);
    try {
      const value = await apiPost<{ uri: string; command: string }>(
        "/api/integrations/signal/link",
        { cli_path: cliPath.trim() || undefined },
      );
      setLinkUri(value.uri);
      const dataUrl = await QRCode.toDataURL(value.uri, {
        margin: 1,
        width: 220,
        color: { dark: "#e4e4e7", light: "#09090b" },
      });
      setQr(dataUrl);
      setMessage(
        "Escaneie o QR no Signal do celular: Configurações → Dispositivos vinculados → Vincular dispositivo.",
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function toggleBridge(start: boolean) {
    setBusy(start ? "start" : "stop");
    setError(null);
    setMessage(null);
    try {
      const value = await apiPost<SignalStatus>(
        `/api/integrations/signal/${start ? "start" : "stop"}`,
      );
      setStatus(value);
      setMessage(start ? "Bridge iniciado." : "Bridge parado.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  const cliOk = status?.cli_available ?? false;
  const running = status?.bridge_running ?? false;

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <MessageCircle size={15} className="text-zinc-500" />
            Signal (celular)
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Converse com um agente pelo Signal. O OpenDesk usa o{" "}
            <span className="font-mono">signal-cli</span> como dispositivo vinculado e
            encaminha mensagens para uma thread de chat.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant={cliOk ? "green" : "red"}>
            cli {cliOk ? "ok" : "ausente"}
          </Badge>
          <Badge variant={running ? "green" : "default"}>
            bridge {running ? "ativo" : "parado"}
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <label className="flex items-center gap-2 text-[11px] text-zinc-400">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(event) => setEnabled(event.target.checked)}
            className="accent-blue-500"
          />
          habilitar integração Signal
        </label>

        <div className="grid gap-2 md:grid-cols-2">
          <label>
            <span className={labelClass}>Número da conta Signal</span>
            <input
              value={account}
              onChange={(event) => setAccount(event.target.value)}
              placeholder="+5511999999999"
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Caminho do signal-cli</span>
            <input
              value={cliPath}
              onChange={(event) => setCliPath(event.target.value)}
              placeholder="signal-cli"
              className={cn(inputClass, "font-mono text-[10px]")}
            />
          </label>
          <label>
            <span className={labelClass}>Workspace padrão</span>
            <select
              value={workspaceId}
              onChange={(event) => setWorkspaceId(event.target.value)}
              className={inputClass}
            >
              <option value="">workspace ativo</option>
              {workspaces.map((workspace) => (
                <option key={workspace.id} value={workspace.id}>
                  {workspace.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className={labelClass}>Agente padrão</span>
            <input
              value={agent}
              onChange={(event) => setAgent(event.target.value)}
              placeholder="build"
              className={inputClass}
            />
          </label>
        </div>

        <label>
          <span className={labelClass}>
            Allowlist de números (separados por vírgula) — obrigatória
          </span>
          <textarea
            value={allowlist}
            onChange={(event) => setAllowlist(event.target.value)}
            rows={2}
            placeholder="+5511999999999, +5511888888888"
            className={cn(inputClass, "h-auto resize-y py-2 font-mono text-[10px]")}
          />
        </label>

        <div className="flex flex-wrap gap-2">
          <Button size="sm" onClick={() => void save()} disabled={busy !== null}>
            {busy === "save" ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
            Salvar
          </Button>
          <Button variant="outline" size="sm" onClick={() => void link()} disabled={busy !== null}>
            {busy === "link" ? <Loader2 size={13} className="animate-spin" /> : <Link2 size={13} />}
            Vincular dispositivo
          </Button>
          {running ? (
            <Button variant="outline" size="sm" onClick={() => void toggleBridge(false)} disabled={busy !== null}>
              <Square size={12} />
              Parar bridge
            </Button>
          ) : (
            <Button variant="outline" size="sm" onClick={() => void toggleBridge(true)} disabled={busy !== null}>
              <Play size={12} />
              Iniciar bridge
            </Button>
          )}
        </div>

        {qr ? (
          <div className="flex flex-wrap items-center gap-4 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
            <img src={qr} alt="QR de vínculo do Signal" className="h-[220px] w-[220px]" />
            <div className="min-w-0 flex-1 space-y-1 text-[10px] text-zinc-500">
              <p className="font-medium text-zinc-300">Escaneie no Signal do celular</p>
              <p>Configurações → Dispositivos vinculados → Vincular dispositivo.</p>
              <p className="break-all font-mono text-[9px] text-zinc-500">{linkUri}</p>
            </div>
          </div>
        ) : null}

        {status && !cliOk ? (
          <p className="flex items-start gap-2 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
            <AlertTriangle size={12} className="mt-0.5 shrink-0" />
            signal-cli não encontrado. Instale com{" "}
            <span className="font-mono">sudo apt install signal-cli</span> ou informe o
            caminho completo.
          </p>
        ) : null}

        {message ? (
          <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[10px] text-emerald-300">
            <Check size={11} className="mr-1 inline" />
            {message}
          </p>
        ) : null}
        {error ? (
          <p className="flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] text-red-300">
            <AlertTriangle size={12} className="mt-0.5 shrink-0" />
            {error}
          </p>
        ) : null}

        <p className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[9px] leading-relaxed text-zinc-500">
          Segurança: o bridge fica desabilitado por padrão, exige allowlist e usa o
          agente escolhido. Mensagens de números fora da allowlist são descartadas.
          Vincular um dispositivo exige o celular com Signal por perto.
        </p>
      </CardContent>
    </Card>
  );
}

