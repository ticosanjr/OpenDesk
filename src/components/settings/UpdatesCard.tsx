import { useEffect, useState } from "react";
import {
  AlertTriangle,
  Check,
  Download,
  KeyRound,
  Loader2,
  Package,
  RefreshCw,
  Search,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import type { DownloadResult, UpdateInfo } from "@/lib/types";

const INSTALLER_LABELS: Record<string, string> = {
  appimage: "AppImage (auto-substituição)",
  portable: "binário portátil",
  system: "pacote do sistema (.deb)",
  development: "build de desenvolvimento",
};

export function UpdatesCard() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [result, setResult] = useState<DownloadResult | null>(null);

  async function load() {
    try {
      const status = await apiGet<UpdateInfo>("/api/updates/status");
      setInfo(status);
      if (status.error) {
        setError(status.error);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function check() {
    setBusy("check");
    setError(null);
    setMessage(null);
    try {
      const status = await apiPost<UpdateInfo>("/api/updates/check");
      setInfo(status);
      setError(status.error ?? null);
      if (!status.error) {
        setMessage(
          status.update_available
            ? `Nova versão disponível: ${status.latest?.version}.`
            : `Você já está na versão mais recente (${status.current}).`,
        );
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function saveToken() {
    if (!token.trim()) {
      return;
    }
    setBusy("token");
    setError(null);
    setMessage(null);
    try {
      const status = await apiPut<UpdateInfo>("/api/updates/token", {
        token: token.trim(),
      });
      setInfo(status);
      setToken("");
      setMessage("Token salvo no cofre local.");
      setError(status.error ?? null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function clearToken() {
    setBusy("token");
    setError(null);
    setMessage(null);
    try {
      const status = await apiDelete<UpdateInfo>("/api/updates/token");
      setInfo(status);
      setMessage("Token removido. O app volta a tentar o gh CLI.");
      setError(status.error ?? null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function download() {
    setBusy("download");
    setError(null);
    setMessage(null);
    setResult(null);
    try {
      const value = await apiPost<DownloadResult>("/api/updates/download");
      setResult(value);
      if (value.ok) {
        setMessage(value.message);
      } else {
        setError(value.message);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  const latest = info?.latest;

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Package size={15} className="text-zinc-500" />
            Atualizações do OpenDesk
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Releases privadas no GitHub. O download é verificado por SHA256 antes
            de substituir o binário; a versão anterior fica como backup.
          </p>
        </div>
        <Badge variant="default">
          {INSTALLER_LABELS[info?.installer ?? ""] ?? info?.installer ?? "—"}
        </Badge>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="grid gap-2 md:grid-cols-3">
          <Info label="Versão atual" value={info?.current ?? "—"} />
          <Info label="Última release" value={latest?.version ?? "—"} />
          <Info
            label="Token GitHub"
            value={info?.token_configured ? "configurado" : "ausente"}
          />
        </div>

        <div className="flex flex-col gap-2 md:flex-row">
          <input
            type="password"
            value={token}
            onChange={(event) => setToken(event.target.value)}
            placeholder="github_pat_… (fine-grained: Contents: Read, Metadata: Read)"
            className="h-9 min-w-0 flex-1 rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 font-mono text-[11px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60"
          />
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              className="h-9"
              onClick={() => void saveToken()}
              disabled={busy !== null || !token.trim()}
            >
              {busy === "token" ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <KeyRound size={13} />
              )}
              Salvar token
            </Button>
            {info?.token_configured ? (
              <Button
                variant="ghost"
                size="sm"
                className="h-9"
                onClick={() => void clearToken()}
                disabled={busy !== null}
              >
                Remover
              </Button>
            ) : null}
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void check()}
            disabled={busy !== null}
          >
            {busy === "check" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Search size={13} />
            )}
            Verificar atualização
          </Button>
          {info?.update_available ? (
            <Button
              size="sm"
              onClick={() => void download()}
              disabled={busy !== null}
            >
              {busy === "download" ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Download size={13} />
              )}
              Baixar e instalar
            </Button>
          ) : null}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void load()}
            disabled={busy !== null}
          >
            <RefreshCw size={13} />
            Atualizar status
          </Button>
        </div>

        {latest?.notes ? (
          <details className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[9px] text-zinc-500">
            <summary className="cursor-pointer text-zinc-400">
              Notas da release {latest.version}
            </summary>
            <p className="mt-2 max-h-32 overflow-y-auto whitespace-pre-wrap leading-relaxed">
              {latest.notes}
            </p>
          </details>
        ) : null}

        {result?.ok ? (
          <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[10px] leading-relaxed text-emerald-300">
            {result.message}
            {result.action === "restart-required"
              ? " Feche o OpenDesk para aplicar."
              : null}
          </p>
        ) : null}

        {message && !result ? (
          <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[10px] leading-relaxed text-emerald-300">
            <Check size={11} className="mr-1 inline" />
            {message}
          </p>
        ) : null}

        {error ? (
          <p className="flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] leading-relaxed text-red-300">
            <AlertTriangle size={12} className="mt-0.5 shrink-0" />
            {error}
          </p>
        ) : null}

        <p className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[9px] leading-relaxed text-zinc-600">
          Repositório privado: crie um token fine-grained em GitHub → Settings →
          Developer settings com acesso ao repositório{" "}
          <span className="font-mono">ticosanjr/OpenDesk</span> e permissões{" "}
          <span className="font-mono">Contents: Read</span> e{" "}
          <span className="font-mono">Metadata: Read</span>. Sem token, o app usa
          o <span className="font-mono">gh auth token</span> quando disponível.
        </p>
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
