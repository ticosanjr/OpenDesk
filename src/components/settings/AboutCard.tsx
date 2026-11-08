import { useEffect, useState } from "react";
import {
  AlertTriangle,
  Check,
  Clipboard,
  Cpu,
  FolderOpen,
  Info,
  Loader2,
  RefreshCw,
  Terminal,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet } from "@/lib/api";
import { shortPath } from "@/lib/format";
import type { AboutInfo } from "@/lib/types";
import { cn } from "@/lib/utils";

export function AboutCard() {
  const [info, setInfo] = useState<AboutInfo | null>(null);
  const [mode, setMode] = useState<"simple" | "advanced">("simple");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<"summary" | "json" | null>(null);

  async function load() {
    setBusy(true);
    try {
      const value = await apiGet<AboutInfo>("/api/about");
      setInfo(value);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function copy(kind: "summary" | "json") {
    if (!info) return;
    const text =
      kind === "json"
        ? JSON.stringify(info, null, 2)
        : [
            `OpenDesk ${info.app.version}`,
            `OpenCode ${info.engine.daemon.version ?? info.engine.resolved?.version ?? "não detectado"} (${info.engine.daemon.mode})`,
            `Daemon: ${info.engine.daemon.online ? "online" : "offline"} · porta ${info.engine.daemon.port}`,
            `Binário: ${info.engine.resolved?.path ?? info.engine.configured_binary ?? "não configurado"}`,
            `Banco: schema ${info.database.schema_version ?? "?"} · ${info.database.workspaces} workspaces`,
            `Runtime: Tauri ${info.runtime.tauri} · ${info.runtime.os}/${info.runtime.arch}`,
            `Instalação: ${info.engine.installer}`,
            `Atualização: token ${info.updates.token_configured ? "configurado" : "ausente"}`,
            `Logs: ${info.paths.log_dir}`,
          ].join("\n");
    try {
      await navigator.clipboard.writeText(text);
      setCopied(kind);
      window.setTimeout(() => setCopied(null), 2000);
    } catch {
      setError("Não foi possível copiar. Use o modo avançado e copie manualmente.");
    }
  }

  const version =
    info?.engine.daemon.version ??
    info?.engine.resolved?.version ??
    "não detectado";

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Info size={15} className="text-zinc-500" />
            Sobre & diagnóstico
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Versões, caminhos e estado do motor. Use o modo simples para suporte e o
            avançado para depuração.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
            {(["simple", "advanced"] as const).map((value) => (
              <button
                key={value}
                type="button"
                onClick={() => setMode(value)}
                className={cn(
                  "rounded-md px-2 py-1 text-[10px]",
                  mode === value
                    ? "bg-zinc-800 text-zinc-100"
                    : "text-zinc-500 hover:text-zinc-300",
                )}
              >
                {value === "simple" ? "simples" : "avançado"}
              </button>
            ))}
          </div>
          <Button variant="ghost" size="sm" onClick={() => void load()} disabled={busy}>
            {busy ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />}
            Atualizar
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] text-red-300">
            {error}
          </p>
        ) : null}

        {!info ? (
          <p className="text-[11px] text-zinc-500">Carregando informações…</p>
        ) : mode === "simple" ? (
          <>
            <div className="grid gap-2 md:grid-cols-3">
              <AboutMetric label="OpenDesk" value={info.app.version} hint={info.app.phase} />
              <AboutMetric
                label="OpenCode"
                value={version}
                hint={info.engine.daemon.online ? info.engine.daemon.mode : "offline"}
                tone={info.engine.daemon.online ? "green" : "red"}
              />
              <AboutMetric
                label="Workspaces"
                value={String(info.database.workspaces)}
                hint={`schema ${info.database.schema_version ?? "?"}`}
              />
              <AboutMetric
                label="Instalação"
                value={info.engine.installer}
                hint={`porta ${info.engine.daemon.port}`}
              />
              <AboutMetric
                label="Runtime"
                value={`Tauri ${info.runtime.tauri}`}
                hint={`${info.runtime.os}/${info.runtime.arch}`}
              />
              <AboutMetric
                label="Atualizações"
                value={info.updates.token_configured ? "token ok" : "sem token"}
                hint={`canal ${info.updates.installer}`}
                tone={info.updates.token_configured ? "green" : "yellow"}
              />
            </div>

            {!info.engine.daemon.online ? (
              <p className="flex items-start gap-2 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] leading-relaxed text-amber-200">
                <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                {info.engine.daemon.message ??
                  "O motor OpenCode está offline. Abra OpenCode Engine para configurar o binário."}
              </p>
            ) : null}

            <div className="flex flex-wrap gap-2">
              <Button variant="outline" size="sm" onClick={() => void copy("summary")}>
                {copied === "summary" ? <Check size={13} /> : <Clipboard size={13} />}
                Copiar resumo
              </Button>
              <Button variant="ghost" size="sm" onClick={() => void copy("json")}>
                {copied === "json" ? <Check size={13} /> : <Clipboard size={13} />}
                Copiar JSON
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void openPath(info.paths.log_dir).catch(() => {})}
              >
                <Terminal size={13} />
                Abrir logs
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void openPath(info.paths.data_dir).catch(() => {})}
              >
                <FolderOpen size={13} />
                Abrir dados
              </Button>
            </div>
          </>
        ) : (
          <>
            <Section title="Aplicação">
              <Row label="Versão" value={info.app.version} />
              <Row label="Fase" value={info.app.phase} />
              <Row label="API" value={info.app.api} />
              <Row label="Porta da API" value={String(info.api_port)} />
              <Row label="Iniciado em" value={info.started_at} />
            </Section>
            <Section title="Runtime">
              <Row label="Tauri" value={info.runtime.tauri} />
              <Row label="Rust" value={info.runtime.rustc ?? "indisponível"} />
              <Row label="OS" value={`${info.runtime.os}/${info.runtime.arch}`} />
              <Row label="Kernel" value={info.runtime.kernel ?? "—"} />
            </Section>
            <Section title="OpenCode">
              <Row
                label="Estado"
                value={`${info.engine.daemon.state} · ${info.engine.daemon.mode}`}
              />
              <Row label="Versão" value={version} />
              <Row label="Porta" value={String(info.engine.daemon.port)} />
              <Row
                label="Binário"
                value={
                  info.engine.resolved?.path ??
                  info.engine.configured_binary ??
                  "não configurado"
                }
              />
              <Row
                label="Método"
                value={info.engine.resolved?.method ?? "desconhecido"}
              />
              {info.engine.daemon.message ? (
                <Row label="Mensagem" value={info.engine.daemon.message} />
              ) : null}
            </Section>
            <Section title="Banco e caminhos">
              <Row label="Schema" value={String(info.database.schema_version ?? "?")} />
              <Row
                label="Tamanho"
                value={`${Math.round(info.database.size_bytes / 1024)} KB`}
              />
              <Row label="DB" value={info.paths.db_path} />
              <Row label="Dados" value={info.paths.data_dir} />
              <Row label="Config" value={info.paths.config_dir} />
              <Row label="Logs" value={info.paths.log_dir} />
              <Row label="Backups" value={info.paths.backups_dir} />
              <Row label="Run logs" value={info.paths.run_logs_dir} />
            </Section>
            <Section title="Atualizações">
              <Row label="Instalação" value={info.updates.installer} />
              <Row label="Versão atual" value={info.updates.current} />
              <Row
                label="Token GitHub"
                value={info.updates.token_configured ? "configurado" : "ausente"}
              />
            </Section>
            <Section title="Arquivos de log">
              {info.logs.length === 0 ? (
                <p className="text-[10px] text-zinc-600">Nenhum log encontrado.</p>
              ) : (
                info.logs.slice(0, 10).map((log) => (
                  <Row
                    key={log.path}
                    label={log.name ?? "log"}
                    value={`${shortPath(log.path, 72)} · ${Math.round(
                      log.size_bytes / 1024,
                    )} KB`}
                  />
                ))
              )}
            </Section>
            <details className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
              <summary className="cursor-pointer text-[10px] text-zinc-400">
                JSON completo
              </summary>
              <pre className="mt-2 max-h-72 overflow-auto whitespace-pre-wrap font-mono text-[9px] text-zinc-400">
                {JSON.stringify(info, null, 2)}
              </pre>
            </details>
          </>
        )}
      </CardContent>
    </Card>
  );
}

function AboutMetric({
  label,
  value,
  hint,
  tone = "default",
}: {
  label: string;
  value: string;
  hint?: string;
  tone?: "default" | "green" | "red" | "yellow";
}) {
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
      <p className="text-[9px] uppercase tracking-wide text-zinc-600">{label}</p>
      <div className="mt-0.5 flex items-center gap-2">
        <p className="truncate font-mono text-[11px] text-zinc-200">{value}</p>
        {tone !== "default" ? (
          <Badge variant={tone === "green" ? "green" : tone === "red" ? "red" : "yellow"}>
            {tone === "green" ? "ok" : tone === "red" ? "offline" : "atenção"}
          </Badge>
        ) : null}
      </div>
      {hint ? <p className="mt-0.5 truncate text-[9px] text-zinc-600">{hint}</p> : null}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
      <p className="mb-2 flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500">
        <Cpu size={11} />
        {title}
      </p>
      <div className="space-y-1">{children}</div>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start justify-between gap-3 text-[10px]">
      <span className="shrink-0 text-zinc-600">{label}</span>
      <span className="min-w-0 break-all text-right font-mono text-zinc-300">
        {value}
      </span>
    </div>
  );
}
