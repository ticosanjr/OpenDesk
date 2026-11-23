import { useCallback, useEffect, useState } from "react";
import { Inbox, Loader2, Pencil, Plus, Trash2 } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { EmptyState } from "@/components/ui/empty-state";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
import { toast } from "@/store/toast-store";
import { relativeTime } from "@/lib/format";

export interface InboundSource {
  id: string;
  name: string;
  source_type: string;
  config_json: string;
  target_agent?: string | null;
  target_column: string;
  target_kind?: string;
  target_id?: string | null;
  auto_execute: boolean;
  is_enabled: boolean;
  last_triggered_at?: string | null;
  last_status?: string | null;
  last_error?: string | null;
}

const TYPES = [
  ["FOLDER_WATCH", "Pasta / Dropzone"],
  ["RSYNC_FOLDER", "Pasta via Rsync (local/remoto)"],
  ["OCR_WATCH", "OCR de imagens/PDF"],
  ["AUDIO_STT_WATCH", "Áudio → texto"],
  ["IMAP_IDLE", "E-mail IMAP"],
  ["WEBHOOK_HTTP", "Webhook / formulário"],
  ["HTTP_POLLER", "HTTP/RSS Poller"],
  ["SQL_POLLER", "SQL Poller"],
  ["TELEGRAM_BOT", "Telegram Bot"],
  ["MATRIX_BOT", "Matrix Bot"],
  ["MATTERMOST_BOT", "Mattermost"],
] as const;

const TEMPLATES: Record<string, string> = {
  FOLDER_WATCH: '{\n  "path": "input/dropzone",\n  "processor": "auto",\n  "settle_secs": 2,\n  "process_empty": false,\n  "process_existing": true,\n  "retry_minutes": 1,\n  "ignore_patterns": []\n}',
  RSYNC_FOLDER: '{\n  "path": "input/dropzone",\n  "transport": "rsync",\n  "rsync_source": "user@host:/srv/inbox",\n  "ssh_key_ref": "rsync-key",\n  "interval_secs": 300\n}',
  OCR_WATCH: '{\n  "path": "input/dropzone",\n  "languages": "por+eng"\n}',
  AUDIO_STT_WATCH: '{\n  "path": "input/dropzone",\n  "whisper_model": "/caminho/model.bin"\n}',
  IMAP_IDLE: '{\n  "host": "imap.exemplo.com",\n  "port": 993,\n  "username": "tasks@exemplo.com",\n  "password_ref": "imap-password",\n  "mailbox": "INBOX",\n  "tls": true\n}',
  WEBHOOK_HTTP: '{\n  "title_path": "$.title",\n  "body_path": "$.body",\n  "priority_path": "$.priority",\n  "secret_ref": "webhook-secret"\n}',
  HTTP_POLLER: '{\n  "url": "https://exemplo.com/feed.xml",\n  "format": "rss",\n  "interval_secs": 300,\n  "selector": "article",\n  "vault_ref": null,\n  "emit_mode": "new_items"\n}',
  SQL_POLLER: '{\n  "database_url_ref": "poller-db",\n  "query": "SELECT * FROM fila WHERE status = \'PENDING\' LIMIT 20",\n  "confirm_query": "UPDATE fila SET status = \'QUEUED\' WHERE id = ?",\n  "id_column": "id",\n  "interval_secs": 60\n}',
  TELEGRAM_BOT: '{\n  "token_ref": "telegram-token",\n  "allowed_chats": []\n}',
  MATRIX_BOT: '{\n  "homeserver": "https://matrix.exemplo.com",\n  "access_token_ref": "matrix-token",\n  "rooms": []\n}',
  MATTERMOST_BOT: '{\n  "token_ref": "mattermost-token"\n}',
};

export function InboundSourcesCard({ workspaceId }: { workspaceId: string }) {
  const [sources, setSources] = useState<InboundSource[]>([]);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState<InboundSource | null>(null);
  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    source_type: "FOLDER_WATCH",
    target_agent: "",
    target_column: "TODO",
    target_kind: "TASK",
    target_id: "",
    auto_execute: true,
    is_enabled: true,
    config: TEMPLATES.FOLDER_WATCH,
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
      const value = await apiGet<{ sources: InboundSource[] }>(
        `/api/io/inbound?workspace_id=${encodeURIComponent(workspaceId)}`,
      );
      setSources(value.sources);
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

  function edit(source?: InboundSource) {
    setEditing(source ?? null);
    setError(null);
    setForm(
      source
        ? {
            name: source.name,
            source_type: source.source_type,
            target_agent: source.target_agent ?? "",
            target_column: source.target_column,
            target_kind: source.target_kind ?? "TASK",
            target_id: source.target_id ?? "",
            auto_execute: source.auto_execute,
            is_enabled: source.is_enabled,
            config: source.config_json,
          }
        : {
            name: "",
            source_type: "FOLDER_WATCH",
            target_agent: "",
            target_column: "TODO",
            target_kind: "TASK",
            target_id: "",
            auto_execute: true,
            is_enabled: true,
            config: TEMPLATES.FOLDER_WATCH,
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
        source_type: form.source_type,
        config,
        target_agent: form.target_agent.trim() || null,
        target_column: form.target_column,
        target_kind: form.target_kind,
        target_id: form.target_id.trim() || null,
        auto_execute: form.auto_execute,
        is_enabled: form.is_enabled,
      };
      if (editing) {
        await apiPatch(`/api/io/inbound/${editing.id}`, body);
      } else {
        await apiPost(
          `/api/io/inbound?workspace_id=${encodeURIComponent(workspaceId)}`,
          body,
        );
      }
      setOpen(false);
      await load();
      toast({ tone: "success", title: editing ? "Gatilho atualizado" : "Gatilho criado" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function remove(source: InboundSource) {
    try {
      await apiDelete(`/api/io/inbound/${source.id}`);
      await load();
      toast({ tone: "success", title: "Gatilho removido" });
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
            <Inbox size={15} className="text-zinc-500" />
            Gatilhos de entrada
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Dropzone, OCR, áudio, IMAP, webhooks, SQL poller e bots convertem tudo
            em tarefas normalizadas.
          </p>
        </div>
        <Button size="sm" onClick={() => edit()}>
          <Plus size={13} />
          Novo gatilho
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
        ) : sources.length === 0 ? (
          <EmptyState
            icon={Inbox}
            title="Nenhum gatilho"
            description="Crie um dropzone, webhook, IMAP ou poller para alimentar o Kanban automaticamente."
          />
        ) : (
          sources.map((source) => (
            <div
              key={source.id}
              className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
            >
              <Badge variant={source.is_enabled ? "green" : "default"}>
                {source.is_enabled ? "ativo" : "pausado"}
              </Badge>
              <Badge variant="purple">{source.source_type}</Badge>
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13.5px] text-zinc-200">{source.name}</p>
                <p className="truncate text-[11.5px] text-zinc-500">
                  {source.target_agent ? `@${source.target_agent} · ` : ""}
                  {source.last_triggered_at
                    ? `${source.last_status ?? "ok"} · ${relativeTime(source.last_triggered_at)}`
                    : "nunca disparou"}
                </p>
                {source.last_error ? (
                  <p className="truncate text-[11.5px] text-red-300/80">{source.last_error}</p>
                ) : null}
              </div>
              <Button variant="ghost" size="icon" className="h-7 w-7" aria-label="Editar fonte" onClick={() => edit(source)}>
                <Pencil size={12} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 text-red-300"
                aria-label="Remover fonte"
                onClick={() => void remove(source)}
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
        title={editing ? "Editar gatilho" : "Novo gatilho"}
        description="A configuração usa JSON. Segredos devem ser referências do SecretStore (`*_ref`)."
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
                value={form.source_type}
                onChange={(event) =>
                  setForm({
                    ...form,
                    source_type: event.target.value,
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
            <label>
              <span className={labelClass}>Agente destino (opcional)</span>
              <input
                value={form.target_agent}
                onChange={(event) => setForm({ ...form, target_agent: event.target.value })}
                placeholder="build"
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Destino</span>
              <select
                value={form.target_kind}
                onChange={(event) =>
                  setForm({ ...form, target_kind: event.target.value })
                }
                className={inputClass}
              >
                <option value="TASK">Tarefa</option>
                <option value="RECIPE">Receita</option>
                <option value="TICKET">Ticket</option>
                <option value="SCHEDULE">Rotina</option>
              </select>
            </label>
            <label>
              <span className={labelClass}>ID do destino</span>
              <input
                value={form.target_id}
                disabled={form.target_kind === "TASK"}
                onChange={(event) =>
                  setForm({ ...form, target_id: event.target.value })
                }
                placeholder="recipe-id / schedule-id"
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Coluna de destino</span>
              <select
                value={form.target_column}
                onChange={(event) => setForm({ ...form, target_column: event.target.value })}
                className={inputClass}
              >
                {["TODO", "IN_PROGRESS", "IN_REVIEW", "BLOCKED"].map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="flex flex-wrap gap-4">
            <label className="flex items-center gap-2 text-[13px] text-zinc-400">
              <input
                type="checkbox"
                checked={form.auto_execute}
                onChange={(event) => setForm({ ...form, auto_execute: event.target.checked })}
              />
              executar automaticamente
            </label>
            <label className="flex items-center gap-2 text-[13px] text-zinc-400">
              <input
                type="checkbox"
                checked={form.is_enabled}
                onChange={(event) => setForm({ ...form, is_enabled: event.target.checked })}
              />
              gatilho ativo
            </label>
          </div>
          {form.source_type === "HTTP_POLLER" ? (
            <div className="grid gap-3 rounded-lg border border-blue-500/20 bg-blue-500/[0.03] p-2.5 sm:grid-cols-2">
              <label className="sm:col-span-2">
                <span className={labelClass}>URL do feed</span>
                <input
                  value={String(configObject().url ?? "")}
                  onChange={(event) => patchConfig("url", event.target.value)}
                  placeholder="https://exemplo.com/feed.xml"
                  className={inputClass}
                />
              </label>
              <label>
                <span className={labelClass}>Formato</span>
                <select
                  value={String(configObject().format ?? "rss")}
                  onChange={(event) => patchConfig("format", event.target.value)}
                  className={inputClass}
                >
                  <option value="rss">RSS</option>
                  <option value="atom">Atom</option>
                  <option value="json">JSON</option>
                  <option value="html">HTML + selector</option>
                </select>
              </label>
              <label>
                <span className={labelClass}>Intervalo (s)</span>
                <input
                  type="number"
                  min={60}
                  value={Number(configObject().interval_secs ?? 300)}
                  onChange={(event) =>
                    patchConfig("interval_secs", Number(event.target.value))
                  }
                  className={inputClass}
                />
              </label>
              <label>
                <span className={labelClass}>Selector HTML</span>
                <input
                  value={String(configObject().selector ?? "")}
                  onChange={(event) => patchConfig("selector", event.target.value)}
                  placeholder="article, .item"
                  className={inputClass}
                />
              </label>
              <label>
                <span className={labelClass}>Vault ref</span>
                <input
                  value={String(configObject().vault_ref ?? "")}
                  onChange={(event) =>
                    patchConfig("vault_ref", event.target.value || null)
                  }
                  placeholder="feed-token"
                  className={inputClass}
                />
              </label>
            </div>
          ) : null}
          {form.source_type === "WEBHOOK_HTTP" ? (
            <div className="grid gap-3 rounded-lg border border-amber-500/20 bg-amber-500/[0.03] p-2.5 sm:grid-cols-2">
              <label>
                <span className={labelClass}>Secret ref</span>
                <input
                  value={String(configObject().secret_ref ?? "")}
                  onChange={(event) =>
                    patchConfig("secret_ref", event.target.value || null)
                  }
                  placeholder="webhook-secret"
                  className={inputClass}
                />
              </label>
              <label>
                <span className={labelClass}>Header da assinatura</span>
                <input
                  value={String(configObject().signature_header ?? "")}
                  onChange={(event) =>
                    patchConfig("signature_header", event.target.value || null)
                  }
                  placeholder="x-opendesk-signature"
                  className={inputClass}
                />
              </label>
            </div>
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
              Salvar gatilho
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
