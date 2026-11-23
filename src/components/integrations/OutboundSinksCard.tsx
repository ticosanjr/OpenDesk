import { useCallback, useEffect, useState } from "react";
import { Loader2, Pencil, Plus, Send, Trash2 } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { EmptyState } from "@/components/ui/empty-state";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
import { toast } from "@/store/toast-store";
import { relativeTime } from "@/lib/format";

export interface OutboundSink {
  id: string;
  name: string;
  sink_type: string;
  config_json: string;
  is_enabled: boolean;
  last_run_at?: string | null;
  last_status?: string | null;
  last_error?: string | null;
}

const TYPES = [
  ["DOC_COMPILER", "Compilar PDF/XLSX"],
  ["ATOMIC_FS", "Pasta/SMB atômica"],
  ["RSYNC_FS", "Pasta via Rsync (local/remoto)"],
  ["MINIO_S3", "MinIO / S3"],
  ["SMTP_EMAIL", "E-mail SMTP"],
  ["TELEGRAM_MSG", "Telegram"],
  ["MATRIX_MSG", "Matrix"],
  ["MATTERMOST_MSG", "Mattermost"],
  ["GIT_COMMIT", "Git commit"],
  ["SQL_INSERT", "SQL insert/upsert"],
  ["HTTP_WEBHOOK", "HTTP Webhook (n8n/CRM)"],
  ["WHATSAPP_MSG", "WhatsApp"],
  ["WEBPUSH", "WebPush"],
] as const;

const TEMPLATES: Record<string, string> = {
  DOC_COMPILER: '{\n  "format": "pdf"\n}',
  ATOMIC_FS: '{\n  "destination": "/caminho/da/pasta"\n}',
  RSYNC_FS: '{\n  "destination": "user@host:/srv/entregas",\n  "transport": "rsync",\n  "ssh_key_ref": "rsync-key",\n  "delete": false,\n  "checksum": true\n}',
  MINIO_S3: '{\n  "bucket": "openDesk",\n  "endpoint": "http://127.0.0.1:9000",\n  "region": "us-east-1",\n  "access_key_ref": "s3-access",\n  "secret_key_ref": "s3-secret",\n  "prefix": "runs"\n}',
  SMTP_EMAIL: '{\n  "to": "ops@exemplo.com",\n  "subject": "[OpenDesk] resultado"\n}',
  TELEGRAM_MSG: '{\n  "token_ref": "telegram-token",\n  "chat_id": "123456"\n}',
  MATRIX_MSG: '{\n  "homeserver": "https://matrix.exemplo.com",\n  "access_token_ref": "matrix-token",\n  "room_id": "!sala:exemplo.com"\n}',
  MATTERMOST_MSG: '{\n  "webhook_url": "https://mattermost/hooks/..."\n}',
  GIT_COMMIT: '{\n  "branch": "opendesk/task",\n  "push": false\n}',
  SQL_INSERT: '{\n  "database_url_ref": "analytics-db",\n  "query": "INSERT INTO resultados (id, resumo) VALUES (?, ?)",\n  "columns": ["id", "resumo"]\n}',
  HTTP_WEBHOOK: '{\n  "url": "https://n8n.exemplo.com/webhook/opendesk",\n  "method": "POST",\n  "headers": {\n    "x-source": "opendesk"\n  },\n  "body_template": "{\\"run_id\\":\\"{{run.id}}\\",\\"summary\\":\\"{{run.summary}}\\"}",\n  "vault_ref": null,\n  "allow_private_network": false\n}',
  WHATSAPP_MSG: '{\n  "to": "5511999999999"\n}',
  WEBPUSH: '{\n  "subject": "OpenDesk"\n}',
};

export function OutboundSinksCard({ workspaceId }: { workspaceId: string }) {
  const [sinks, setSinks] = useState<OutboundSink[]>([]);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState<OutboundSink | null>(null);
  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    sink_type: "DOC_COMPILER",
    is_enabled: true,
    config: TEMPLATES.DOC_COMPILER,
  });

  function configObject(): Record<string, unknown> {
    try {
      const parsed = JSON.parse(form.config);
      return parsed && typeof parsed === "object" && !Array.isArray(parsed)
        ? (parsed as Record<string, unknown>)
        : {};
    } catch {
      return {};
    }
  }

  function patchConfig(key: string, value: unknown) {
    const current = configObject();
    current[key] = value;
    setForm({ ...form, config: JSON.stringify(current, null, 2) });
  }

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      const value = await apiGet<{ sinks: OutboundSink[] }>(
        `/api/io/outbound?workspace_id=${encodeURIComponent(workspaceId)}`,
      );
      setSinks(value.sinks);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  function edit(sink?: OutboundSink) {
    setEditing(sink ?? null);
    setError(null);
    setForm(
      sink
        ? {
            name: sink.name,
            sink_type: sink.sink_type,
            is_enabled: sink.is_enabled,
            config: sink.config_json,
          }
        : {
            name: "",
            sink_type: "DOC_COMPILER",
            is_enabled: true,
            config: TEMPLATES.DOC_COMPILER,
          },
    );
    setOpen(true);
  }

  async function save() {
    if (!form.name.trim()) {
      setError("Informe um nome.");
      return;
    }
    let config: unknown;
    try {
      config = JSON.parse(form.config || "{}");
    } catch {
      setError("Config JSON inválido.");
      return;
    }
    setSaving(true);
    try {
      const body = {
        name: form.name.trim(),
        sink_type: form.sink_type,
        config,
        is_enabled: form.is_enabled,
      };
      if (editing) {
        await apiPatch(`/api/io/outbound/${editing.id}`, body);
      } else {
        await apiPost(
          `/api/io/outbound?workspace_id=${encodeURIComponent(workspaceId)}`,
          body,
        );
      }
      setOpen(false);
      await load();
      toast({ tone: "success", title: editing ? "Sink atualizado" : "Sink criado" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function remove(sink: OutboundSink) {
    try {
      await apiDelete(`/api/io/outbound/${sink.id}`);
      await load();
      toast({ tone: "success", title: "Sink removido" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao remover",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Send size={15} className="text-zinc-500" />
            Entregas automáticas (sinks)
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Compile PDF/XLSX, grave em pasta/S3, envie por SMTP/chat, faça commit
            Git ou insira no SQL — sem depender do LLM.
          </p>
        </div>
        <Button size="sm" onClick={() => edit()}>
          <Plus size={13} />
          Novo sink
        </Button>
      </CardHeader>
      <CardContent className="space-y-2">
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
            {error}
          </p>
        ) : null}
        {loading ? (
          <p className="flex items-center gap-2 p-3 text-[13px] text-zinc-500">
            <Loader2 size={12} className="animate-spin" /> Carregando…
          </p>
        ) : sinks.length === 0 ? (
          <EmptyState
            icon={Send}
            title="Nenhum sink"
            description="Crie um destino e vincule-o a uma tarefa ou rotina no Kanban."
          />
        ) : (
          sinks.map((sink) => (
            <div
              key={sink.id}
              className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
            >
              <Badge variant={sink.is_enabled ? "green" : "default"}>
                {sink.is_enabled ? "ativo" : "pausado"}
              </Badge>
              <Badge variant="purple">{sink.sink_type}</Badge>
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13.5px] text-zinc-200">{sink.name}</p>
                <p className="truncate text-[11.5px] text-zinc-500">
                  {sink.last_run_at
                    ? `${sink.last_status ?? "ok"} · ${relativeTime(sink.last_run_at)}`
                    : "nunca executou"}
                </p>
                {sink.last_error ? (
                  <p className="truncate text-[11.5px] text-red-300/80">{sink.last_error}</p>
                ) : null}
              </div>
              <Button variant="ghost" size="icon" className="h-7 w-7" aria-label="Editar sink" onClick={() => edit(sink)}>
                <Pencil size={12} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 text-red-300"
                aria-label="Remover sink"
                onClick={() => void remove(sink)}
              >
                <Trash2 size={12} />
              </Button>
            </div>
          ))
        )}
      </CardContent>

      <Modal
        open={open}
        onClose={() => setOpen(false)}
        title={editing ? "Editar sink" : "Novo sink"}
        description="O sink roda depois de um run SUCCESS. Use referências de segredo (`*_ref`) no JSON."
        size="lg"
      >
        <div className="space-y-3">
          <div className="grid gap-3 sm:grid-cols-2">
            <label>
              <span className={labelClass}>Nome</span>
              <input
                value={form.name}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Tipo</span>
              <select
                value={form.sink_type}
                onChange={(event) =>
                  setForm({
                    ...form,
                    sink_type: event.target.value,
                    config: TEMPLATES[event.target.value] ?? form.config,
                  })
                }
                className={inputClass}
              >
                {TYPES.map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <label className="flex items-center gap-2 text-[13px] text-zinc-400">
            <input
              type="checkbox"
              checked={form.is_enabled}
              onChange={(event) => setForm({ ...form, is_enabled: event.target.checked })}
            />
            sink ativo
          </label>
          {form.sink_type === "HTTP_WEBHOOK" ? (
            <div className="space-y-3 rounded-lg border border-blue-500/20 bg-blue-500/[0.03] p-2.5">
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="sm:col-span-2">
                  <span className={labelClass}>URL do webhook</span>
                  <input
                    value={String(configObject().url ?? "")}
                    onChange={(event) => patchConfig("url", event.target.value)}
                    placeholder="https://n8n.exemplo.com/webhook/opendesk"
                    className={inputClass}
                  />
                </label>
                <label>
                  <span className={labelClass}>Método</span>
                  <select
                    value={String(configObject().method ?? "POST")}
                    onChange={(event) => patchConfig("method", event.target.value)}
                    className={inputClass}
                  >
                    <option value="POST">POST</option>
                    <option value="PUT">PUT</option>
                    <option value="PATCH">PATCH</option>
                  </select>
                </label>
                <label>
                  <span className={labelClass}>Vault ref (bearer)</span>
                  <input
                    value={String(configObject().vault_ref ?? "")}
                    onChange={(event) =>
                      patchConfig("vault_ref", event.target.value || null)
                    }
                    placeholder="n8n-token"
                    className={inputClass}
                  />
                </label>
              </div>
              <label className="flex items-center gap-2 text-[12.5px] text-zinc-400">
                <input
                  type="checkbox"
                  checked={Boolean(configObject().allow_private_network)}
                  onChange={(event) =>
                    patchConfig("allow_private_network", event.target.checked)
                  }
                />
                permitir rede privada (n8n/local)
              </label>
              <label className="block">
                <span className={labelClass}>Body template</span>
                <textarea
                  value={String(configObject().body_template ?? "")}
                  onChange={(event) =>
                    patchConfig("body_template", event.target.value)
                  }
                  rows={4}
                  placeholder='{"run_id":"{{run.id}}","summary":"{{run.summary}}"}'
                  className="w-full rounded-lg border border-zinc-800 bg-zinc-900/50 p-2 font-mono text-[11.5px] text-zinc-200 outline-none focus:border-blue-500/60"
                />
              </label>
            </div>
          ) : null}
          {form.sink_type === "WHATSAPP_MSG" ? (
            <label className="block rounded-lg border border-emerald-500/20 bg-emerald-500/[0.03] p-2.5">
              <span className={labelClass}>Destinatário (E.164)</span>
              <input
                value={String(configObject().to ?? "")}
                onChange={(event) => patchConfig("to", event.target.value)}
                placeholder="5511999999999"
                className={inputClass}
              />
            </label>
          ) : null}
          {form.sink_type === "WEBPUSH" ? (
            <label className="block rounded-lg border border-violet-500/20 bg-violet-500/[0.03] p-2.5">
              <span className={labelClass}>Título da notificação</span>
              <input
                value={String(configObject().subject ?? "")}
                onChange={(event) => patchConfig("subject", event.target.value)}
                placeholder="OpenDesk"
                className={inputClass}
              />
            </label>
          ) : null}
          <label className="block">
            <span className={labelClass}>Config JSON</span>
            <textarea
              value={form.config}
              onChange={(event) => setForm({ ...form, config: event.target.value })}
              rows={10}
              className="w-full rounded-lg border border-zinc-800 bg-zinc-900/50 p-2 font-mono text-[12px] text-zinc-200 outline-none focus:border-blue-500/60"
            />
          </label>
          {error ? <p className="text-[13px] text-red-300">{error}</p> : null}
          <div className="flex justify-end gap-2 border-t border-zinc-800/70 pt-3">
            <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
              Cancelar
            </Button>
            <Button size="sm" onClick={() => void save()} disabled={saving}>
              {saving ? <Loader2 size={13} className="animate-spin" /> : null}
              Salvar sink
            </Button>
          </div>
        </div>
      </Modal>
    </Card>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[13.5px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60";
const labelClass =
  "mb-1 block text-[11.5px] font-medium uppercase tracking-[0.12em] text-zinc-500";
