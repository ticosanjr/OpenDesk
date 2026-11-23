import { useCallback, useEffect, useState } from "react";
import { KeyRound, Loader2, Pencil, Plus, ShieldAlert, Trash2, Upload } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { EmptyState } from "@/components/ui/empty-state";
import { apiDelete, apiGet, apiPatch, apiPost, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";
import { relativeTime } from "@/lib/format";
import { domainsOf, parseAcls } from "./vault-utils";
import { VaultBitwardenButton } from "./VaultBitwardenButton";

export interface VaultEntry {
  id: string;
  workspace_id?: string | null;
  kind: string;
  label: string;
  username?: string | null;
  allowed_domains_json: string;
  requires_hitl: boolean;
  expires_at?: string | null;
  last_used_at?: string | null;
  created_at: string;
  updated_at: string;
}

interface VaultAudit {
  id: string;
  actor_kind: string;
  actor_id: string;
  outcome: string;
  target_domain?: string | null;
  created_at: string;
}

const KINDS = [
  ["api_key", "API key"],
  ["login", "Login / senha"],
  ["bearer_token", "Bearer token"],
  ["totp_seed", "TOTP (seed)"],
  ["ssh_key", "Chave SSH"],
  ["header_bundle", "Headers"],
] as const;

const EMPTY_FORM = {
  id: "",
  kind: "api_key",
  label: "",
  username: "",
  domains: "",
  secret: "",
  requires_hitl: true,
  expires_at: "",
  acls: "agent:build",
};

export function VaultCard({ workspaceId }: { workspaceId: string }) {
  const [entries, setEntries] = useState<VaultEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState<VaultEntry | null>(null);
  const [form, setForm] = useState(EMPTY_FORM);
  const [saving, setSaving] = useState(false);
  const [auditOpen, setAuditOpen] = useState(false);
  const [auditRows, setAuditRows] = useState<VaultAudit[]>([]);
  const [importOpen, setImportOpen] = useState(false);
  const [importText, setImportText] = useState("");
  const [importing, setImporting] = useState(false);

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      const value = await apiGet<{ entries: VaultEntry[] }>(
        `/api/vault?workspace_id=${encodeURIComponent(workspaceId)}`,
      );
      setEntries(value.entries);
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

  function openCreate() {
    setEditing(null);
    setForm(EMPTY_FORM);
    setError(null);
    setOpen(true);
  }

  function openEdit(entry: VaultEntry) {
    setEditing(entry);
    setForm({
      id: entry.id,
      kind: entry.kind,
      label: entry.label,
      username: entry.username ?? "",
      domains: domainsOf(entry).join(", "),
      secret: "",
      requires_hitl: entry.requires_hitl,
      expires_at: entry.expires_at ?? "",
      acls: "",
    });
    setError(null);
    setOpen(true);
  }

  async function save() {
    setSaving(true);
    setError(null);
    const allowedDomains = form.domains
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean);
    try {
      if (editing) {
        await apiPatch(`/api/vault/${encodeURIComponent(editing.id)}`, {
          label: form.label,
          username: form.username || null,
          allowed_domains: allowedDomains,
          requires_hitl: form.requires_hitl,
          expires_at: form.expires_at || null,
          secret: form.secret || undefined,
        });
        if (form.acls.trim()) {
          await apiPut(`/api/vault/${encodeURIComponent(editing.id)}/acl`, {
            acls: parseAcls(form.acls),
          });
        }
      } else {
        await apiPost(`/api/vault?workspace_id=${encodeURIComponent(workspaceId)}`, {
          id: form.id,
          kind: form.kind,
          label: form.label,
          username: form.username || null,
          allowed_domains: allowedDomains,
          secret: form.secret,
          requires_hitl: form.requires_hitl,
          expires_at: form.expires_at || null,
          acls: parseAcls(form.acls),
        });
      }
      setOpen(false);
      await load();
      toast({ tone: "success", title: editing ? "Credencial atualizada" : "Credencial criada" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function remove(entry: VaultEntry) {
    if (!window.confirm(`Excluir a credencial "${entry.label}"?`)) return;
    try {
      await apiDelete(`/api/vault/${encodeURIComponent(entry.id)}`);
      await load();
      toast({ tone: "success", title: "Credencial excluída" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha na credencial",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function openAudit(entry: VaultEntry) {
    try {
      const value = await apiGet<{ audit: VaultAudit[] }>(
        `/api/vault/audit?entry_id=${encodeURIComponent(entry.id)}&limit=30`,
      );
      setAuditRows(value.audit);
      setAuditOpen(true);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha na credencial",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function importEnv() {
    setImporting(true);
    try {
      const value = await apiPost<{ imported: VaultEntry[] }>("/api/vault/import-env", {
        content: importText,
        workspace_id: workspaceId,
      });
      setImportOpen(false);
      setImportText("");
      await load();
      toast({
        tone: "success",
        title: `${value.imported.length} credencial(is) importada(s)`,
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha na credencial",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setImporting(false);
    }
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-3">
        <div>
          <CardTitle className="flex items-center gap-2">
            <KeyRound size={16} /> Credenciais
          </CardTitle>
          <p className="mt-1 text-[12.5px] text-zinc-500">
            O segredo fica cifrado no banco e nunca é devolvido ao agente. Injeção só nas bordas
            HTTP/MCP; login no navegador usa HITL.
          </p>
        </div>
        <div className="flex gap-2">
          <VaultBitwardenButton workspaceId={workspaceId} onImported={() => void load()} />
          <Button variant="ghost" size="sm" onClick={() => setImportOpen(true)}>
            <Upload size={13} /> .env
          </Button>
          <Button size="sm" onClick={openCreate}>
            <Plus size={13} /> Nova
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {error ? <p className="text-[13px] text-red-300">{error}</p> : null}
        {loading ? (
          <div className="flex items-center gap-2 text-[13px] text-zinc-500">
            <Loader2 size={14} className="animate-spin" /> Carregando credenciais...
          </div>
        ) : entries.length === 0 ? (
          <EmptyState
            title="Nenhuma credencial"
            description="Crie uma entrada ou importe um .env. Sem ACL, a credencial não pode ser injetada."
            icon={KeyRound}
          />
        ) : (
          entries.map((entry) => (
            <div
              key={entry.id}
              className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-zinc-800/70 bg-zinc-900/30 px-3 py-2.5"
            >
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-[13.5px] font-medium text-zinc-100">{entry.label}</span>
                  <Badge variant="purple">{entry.kind}</Badge>
                  {entry.requires_hitl ? (
                    <Badge variant="yellow">
                      <ShieldAlert size={11} className="mr-1" /> HITL
                    </Badge>
                  ) : (
                    <Badge variant="green">automático</Badge>
                  )}
                  {entry.expires_at ? <Badge variant="red">expira</Badge> : null}
                </div>
                <p className="mt-1 font-mono text-[11px] text-zinc-500">
                  {entry.id} · {domainsOf(entry).join(", ") || "sem domínio"}
                  {entry.last_used_at ? ` · usado ${relativeTime(entry.last_used_at)}` : ""}
                </p>
              </div>
              <div className="flex items-center gap-1">
                <Button variant="ghost" size="sm" onClick={() => void openAudit(entry)}>
                  Auditoria
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7"
                  aria-label="Editar credencial"
                  onClick={() => openEdit(entry)}
                >
                  <Pencil size={12} />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 text-red-300"
                  aria-label="Excluir credencial"
                  onClick={() => void remove(entry)}
                >
                  <Trash2 size={12} />
                </Button>
              </div>
            </div>
          ))
        )}
      </CardContent>

      <Modal
        open={open}
        onClose={() => setOpen(false)}
        title={editing ? "Editar credencial" : "Nova credencial"}
        description="O valor é cifrado com a chave da máquina. A UI nunca relê o segredo; para trocar, informe um novo."
        size="lg"
      >
        <div className="space-y-3">
          <div className="grid gap-3 sm:grid-cols-2">
            <label>
              <span className={labelClass}>Id</span>
              <input
                value={form.id}
                disabled={Boolean(editing)}
                onChange={(event) => setForm({ ...form, id: event.target.value })}
                placeholder="stripe-live"
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Tipo</span>
              <select
                value={form.kind}
                onChange={(event) => setForm({ ...form, kind: event.target.value })}
                className={inputClass}
              >
                {KINDS.map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span className={labelClass}>Rótulo</span>
              <input
                value={form.label}
                onChange={(event) => setForm({ ...form, label: event.target.value })}
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Usuário (opcional)</span>
              <input
                value={form.username}
                onChange={(event) => setForm({ ...form, username: event.target.value })}
                className={inputClass}
              />
            </label>
            <label className="sm:col-span-2">
              <span className={labelClass}>Domínios permitidos</span>
              <input
                value={form.domains}
                onChange={(event) => setForm({ ...form, domains: event.target.value })}
                placeholder="api.stripe.com, portal.fornecedor.com"
                className={inputClass}
              />
            </label>
            <label className="sm:col-span-2">
              <span className={labelClass}>
                {editing ? "Novo segredo (opcional)" : "Segredo"}
              </span>
              <input
                type="password"
                value={form.secret}
                onChange={(event) => setForm({ ...form, secret: event.target.value })}
                className={inputClass}
                autoComplete="new-password"
              />
            </label>
            <label>
              <span className={labelClass}>Expira em (RFC3339, opcional)</span>
              <input
                value={form.expires_at}
                onChange={(event) => setForm({ ...form, expires_at: event.target.value })}
                placeholder="2027-01-01T00:00:00Z"
                className={inputClass}
              />
            </label>
            <label className="flex items-end gap-2 pb-2 text-[13px] text-zinc-400">
              <input
                type="checkbox"
                checked={form.requires_hitl}
                onChange={(event) =>
                  setForm({ ...form, requires_hitl: event.target.checked })
                }
              />
              exigir HITL antes de injetar
            </label>
            <label className="sm:col-span-2">
              <span className={labelClass}>ACL (linhas tipo:id)</span>
              <textarea
                value={form.acls}
                onChange={(event) => setForm({ ...form, acls: event.target.value })}
                rows={3}
                placeholder={"agent:build\nrecipe:minha-receita\nsink:meu-sink"}
                className="w-full rounded-lg border border-zinc-800 bg-zinc-900/50 p-2 font-mono text-[12px] text-zinc-200 outline-none focus:border-blue-500/60"
              />
              <span className="mt-1 block text-[11.5px] text-zinc-500">
                Sem ACL a injeção é negada. Deixe em branco ao editar para preservar as ACLs atuais.
              </span>
            </label>
          </div>
          {error ? <p className="text-[13px] text-red-300">{error}</p> : null}
          <div className="flex justify-end gap-2 border-t border-zinc-800/70 pt-3">
            <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
              Cancelar
            </Button>
            <Button size="sm" onClick={() => void save()} disabled={saving}>
              {saving ? <Loader2 size={13} className="animate-spin" /> : null} Salvar
            </Button>
          </div>
        </div>
      </Modal>

      <Modal
        open={auditOpen}
        onClose={() => setAuditOpen(false)}
        title="Auditoria da credencial"
        description="Toda tentativa de injeção é registrada, inclusive as negadas."
        size="lg"
      >
        <div className="space-y-2">
          {auditRows.length === 0 ? (
            <p className="text-[13px] text-zinc-500">Nenhum uso registrado.</p>
          ) : (
            auditRows.map((row) => (
              <div
                key={row.id}
                className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 px-3 py-2 text-[12.5px]"
              >
                <span className="font-mono text-zinc-300">{row.outcome}</span>
                <span className="text-zinc-500">
                  {row.actor_kind}:{row.actor_id}
                  {row.target_domain ? ` → ${row.target_domain}` : ""} ·{" "}
                  {relativeTime(row.created_at)}
                </span>
              </div>
            ))
          )}
        </div>
      </Modal>

      <Modal
        open={importOpen}
        onClose={() => setImportOpen(false)}
        title="Importar .env"
        description="Cada linha KEY=VALUE vira uma credencial. Segredos curtos e chaves existentes são ignorados."
        size="md"
      >
        <div className="space-y-3">
          <textarea
            value={importText}
            onChange={(event) => setImportText(event.target.value)}
            rows={8}
            className="w-full rounded-lg border border-zinc-800 bg-zinc-900/50 p-2 font-mono text-[12px] text-zinc-200 outline-none focus:border-blue-500/60"
            placeholder="STRIPE_KEY=..."
          />
          <div className="flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={() => setImportOpen(false)}>
              Cancelar
            </Button>
            <Button size="sm" onClick={() => void importEnv()} disabled={importing}>
              {importing ? <Loader2 size={13} className="animate-spin" /> : null} Importar
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
