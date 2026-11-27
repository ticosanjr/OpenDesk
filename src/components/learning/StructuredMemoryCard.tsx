import { useCallback, useEffect, useState } from "react";
import { BrainCircuit, Download, Loader2, Plus, ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import {
  StructuredMemoryRow,
  type MemoryNote,
} from "@/components/learning/StructuredMemoryRow";
import { JsonImportButton } from "@/components/ui/json-import-button";
import { exportJsonBundle, type BundleEnvelope } from "@/lib/import-export";
import { toast } from "@/store/toast-store";
import type { MemoryUsageScope } from "@/lib/types";
import {
  MemoryScopeFilterBar,
  matchesScopeFilter,
  type MemoryScopeFilter,
} from "@/components/learning/MemoryScopeControls";

interface MemoryNamespace {
  id: string;
  name: string;
  description?: string | null;
}

interface MemoryAcl {
  namespace_id: string;
  agent: string;
  can_read: boolean;
  can_write: boolean;
}

const TRUST = ["observed", "verified", "inferred"];
const KINDS = ["semantic", "episodic", "procedural", "preference"];
const inputClass =
  "h-8 w-full rounded border border-zinc-700 bg-zinc-950 px-2 text-[12.5px] text-zinc-200";

export function StructuredMemoryCard({ workspaceId }: { workspaceId: string }) {
  const [notes, setNotes] = useState<MemoryNote[]>([]);
  const [namespaces, setNamespaces] = useState<MemoryNamespace[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [selectedNamespace, setSelectedNamespace] = useState("");
  const [scopeFilter, setScopeFilter] = useState<MemoryScopeFilter>("all");
  const [acl, setAcl] = useState<MemoryAcl[]>([]);
  const [aclForm, setAclForm] = useState({ agent: "", read: true, write: false });
  const [form, setForm] = useState({
    namespace: "core",
    kind: "semantic",
    title: "",
    body: "",
    tags: "",
    confidence: "0.6",
    trust: "observed",
    usageScope: "normal" as MemoryUsageScope,
  });

  const query = `?workspace_id=${encodeURIComponent(workspaceId)}`;

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      const [noteList, namespaceList] = await Promise.all([
        apiGet<MemoryNote[]>(`/api/memory/notes${query}&limit=100`),
        apiGet<MemoryNamespace[]>(`/api/memory/namespaces${query}`),
      ]);
      setNotes(noteList);
      setNamespaces(namespaceList);
      setSelectedNamespace((current) => current || namespaceList[0]?.name || "");
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar a memória",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [workspaceId, query]);

  useEffect(() => {
    void load();
  }, [load]);

  const selected = namespaces.find((item) => item.name === selectedNamespace);

  useEffect(() => {
    if (!selected) {
      setAcl([]);
      return;
    }
    void apiGet<MemoryAcl[]>(`/api/memory/namespaces/${selected.id}/acl`)
      .then(setAcl)
      .catch(() => setAcl([]));
  }, [selected]);

  async function addNote() {
    if (!form.title.trim() || !form.body.trim()) {
      toast({ tone: "error", title: "Título e corpo são obrigatórios" });
      return;
    }
    setBusy("note");
    try {
      await apiPost(`/api/memory/notes${query}`, {
        namespace: form.namespace.trim() || "core",
        kind: form.kind,
        title: form.title.trim(),
        body: form.body.trim(),
        tags: form.tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
        confidence: Number(form.confidence) || 0.5,
        trust_level: form.trust,
        usage_scope: form.usageScope,
      });
      setForm({ ...form, title: "", body: "", tags: "" });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao salvar a memória",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function updateScope(id: string, usageScope: MemoryUsageScope) {
    setBusy(`scope:${id}`);
    try {
      await apiPut(`/api/memory/notes/${id}`, { usage_scope: usageScope });
      await load();
      toast({
        tone: "success",
        title: `Escopo alterado para ${usageScope === "both" ? "ambos" : usageScope}`,
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao alterar o escopo",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function removeNote(id: string) {
    setBusy(`note:${id}`);
    try {
      await apiDelete(`/api/memory/notes/${id}`);
      await load();
    } finally {
      setBusy(null);
    }
  }

  async function saveAcl() {
    if (!selected || !aclForm.agent.trim()) return;
    setBusy("acl");
    try {
      await apiPut(`/api/memory/namespaces/${selected.id}/acl`, {
        agent: aclForm.agent.trim(),
        can_read: aclForm.read,
        can_write: aclForm.write,
      });
      setAclForm({ agent: "", read: true, write: false });
      setAcl(
        await apiGet<MemoryAcl[]>(`/api/memory/namespaces/${selected.id}/acl`),
      );
      toast({ tone: "success", title: "ACL atualizada" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao salvar ACL",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function removeAcl(agent: string) {
    if (!selected) return;
    setBusy(`acl:${agent}`);
    try {
      await apiDelete(`/api/memory/namespaces/${selected.id}/acl/${agent}`);
      setAcl(
        await apiGet<MemoryAcl[]>(`/api/memory/namespaces/${selected.id}/acl`),
      );
    } finally {
      setBusy(null);
    }
  }

  async function exportMemory() {
    if (!workspaceId) return;
    setBusy("export");
    try {
      const bundle = await apiGet<BundleEnvelope>(`/api/memory/export${query}`);
      const savedName = await exportJsonBundle(
        "structured_memory",
        bundle.items,
        `opendesk-structured-memory-${new Date().toISOString().slice(0, 10)}.json`,
        workspaceId,
      );
      if (!savedName) return;
      toast({ tone: "success", title: "Memória estruturada exportada" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível exportar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  const visibleNotes = notes.filter((note) =>
    matchesScopeFilter(note.usage_scope, scopeFilter),
  );

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <BrainCircuit size={15} className="text-zinc-500" />
            Memória estruturada
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Notas reutilizáveis por namespace, com confiança, validade e ACL por
            agente. O bloco relevante é injetado no prompt de quem pode ler.
          </p>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex flex-wrap justify-end gap-1.5">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void exportMemory()}
            disabled={!workspaceId || busy === "export"}
          >
            {busy === "export" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Download size={13} />
            )}
            Exportar JSON
          </Button>
          <JsonImportButton
            kind="structured_memory"
            endpoint="/api/memory/import"
            workspaceId={workspaceId}
            disabled={!workspaceId}
            onImported={load}
          />
        </div>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <MemoryScopeFilterBar value={scopeFilter} onChange={setScopeFilter} />
          <span className="text-[11px] text-zinc-500">
            Normal = tarefas/rotinas · Chat = conversas e bots · Ambos = os dois
          </span>
        </div>
        <div className="grid gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 sm:grid-cols-2 lg:grid-cols-4">
          <input
            value={form.namespace}
            onChange={(event) => setForm({ ...form, namespace: event.target.value })}
            placeholder="namespace"
            className={inputClass}
            list="memory-namespaces"
          />
          <datalist id="memory-namespaces">
            {namespaces.map((namespace) => (
              <option key={namespace.id} value={namespace.name} />
            ))}
          </datalist>
          <select
            aria-label="Tipo da memória"
            value={form.kind}
            onChange={(event) => setForm({ ...form, kind: event.target.value })}
            className={inputClass}
          >
            {KINDS.map((kind) => (
              <option key={kind} value={kind}>
                {kind}
              </option>
            ))}
          </select>
          <select
            aria-label="Nível de confiança"
            value={form.trust}
            onChange={(event) => setForm({ ...form, trust: event.target.value })}
            className={inputClass}
          >
            {TRUST.map((trust) => (
              <option key={trust} value={trust}>
                {trust}
              </option>
            ))}
          </select>
          <select
            aria-label="Escopo de uso da memória"
            value={form.usageScope}
            onChange={(event) =>
              setForm({ ...form, usageScope: event.target.value as MemoryUsageScope })
            }
            className={inputClass}
          >
            <option value="normal">Normal</option>
            <option value="chat">Chat</option>
            <option value="both">Ambos</option>
          </select>
          <input
            value={form.confidence}
            onChange={(event) => setForm({ ...form, confidence: event.target.value })}
            placeholder="0.0–1.0"
            inputMode="decimal"
            className={inputClass}
          />
          <input
            value={form.title}
            onChange={(event) => setForm({ ...form, title: event.target.value })}
            placeholder="título"
            className={`${inputClass} lg:col-span-2`}
          />
          <input
            value={form.tags}
            onChange={(event) => setForm({ ...form, tags: event.target.value })}
            placeholder="tags (vírgula)"
            className={inputClass}
          />
          <textarea
            value={form.body}
            onChange={(event) => setForm({ ...form, body: event.target.value })}
            rows={2}
            placeholder="fato, regra ou aprendizado reutilizável"
            className={`${inputClass} sm:col-span-2 lg:col-span-3`}
          />
          <div className="flex items-end justify-end">
            <Button
              size="sm"
              className="h-8"
              disabled={busy === "note"}
              onClick={() => void addNote()}
            >
              {busy === "note" ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Plus size={12} />
              )}
              Salvar nota
            </Button>
          </div>
        </div>

        {loading ? (
          <p className="text-[12.5px] text-zinc-500">carregando memória…</p>
        ) : visibleNotes.length === 0 ? (
          <EmptyState
            title={notes.length === 0 ? "Memória vazia" : "Nenhuma nota neste filtro"}
            description={
              notes.length === 0
                ? "Agentes podem salvar notas via MCP `memory_create`; você também pode criar aqui."
                : "Troque o filtro de escopo para ver outras notas."
            }
          />
        ) : (
          <div className="space-y-1.5">
            {visibleNotes.map((note) => (
              <StructuredMemoryRow
                key={note.id}
                note={note}
                busy={busy}
                onScopeChange={(value) => void updateScope(note.id, value)}
                onRemove={() => void removeNote(note.id)}
              />
            ))}
          </div>
        )}

        <div className="space-y-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
          <div className="flex flex-wrap items-center gap-2">
            <ShieldCheck size={13} className="text-zinc-500" />
            <span className="text-[12.5px] font-medium text-zinc-300">
              ACL do namespace
            </span>
            <select
              aria-label="Namespace da ACL"
              value={selectedNamespace}
              onChange={(event) => setSelectedNamespace(event.target.value)}
              className={`${inputClass} w-44`}
            >
              <option value="">selecione…</option>
              {namespaces.map((namespace) => (
                <option key={namespace.id} value={namespace.name}>
                  {namespace.name}
                </option>
              ))}
            </select>
            <span className="text-[11.5px] text-zinc-500">
              sem linhas = aberto a todos
            </span>
          </div>
          {selected ? (
            <>
              <div className="flex flex-wrap items-center gap-2">
                <input
                  value={aclForm.agent}
                  onChange={(event) =>
                    setAclForm({ ...aclForm, agent: event.target.value })
                  }
                  placeholder="agente"
                  className={`${inputClass} w-32`}
                />
                <label className="flex items-center gap-1 text-[12px] text-zinc-400">
                  <input
                    type="checkbox"
                    checked={aclForm.read}
                    onChange={(event) =>
                      setAclForm({ ...aclForm, read: event.target.checked })
                    }
                    className="h-3.5 w-3.5 accent-blue-500"
                  />
                  ler
                </label>
                <label className="flex items-center gap-1 text-[12px] text-zinc-400">
                  <input
                    type="checkbox"
                    checked={aclForm.write}
                    onChange={(event) =>
                      setAclForm({ ...aclForm, write: event.target.checked })
                    }
                    className="h-3.5 w-3.5 accent-blue-500"
                  />
                  escrever
                </label>
                <Button
                  size="sm"
                  className="h-7"
                  aria-label="Aplicar ACL"
                  disabled={busy === "acl"}
                  onClick={() => void saveAcl()}
                >
                  Aplicar
                </Button>
              </div>
              {acl.length > 0 ? (
                <div className="flex flex-wrap gap-1.5">
                  {acl.map((entry) => (
                    <span
                      key={entry.agent}
                      className="inline-flex items-center gap-1 rounded border border-zinc-800 bg-zinc-900/60 px-1.5 py-0.5 font-mono text-[11px] text-zinc-400"
                    >
                      @{entry.agent}: {entry.can_read ? "R" : "-"}
                      {entry.can_write ? "W" : "-"}
                      <button
                        type="button"
                        disabled={busy === `acl:${entry.agent}`}
                        onClick={() => void removeAcl(entry.agent)}
                        className="text-zinc-600 hover:text-red-300"
                      >
                        ×
                      </button>
                    </span>
                  ))}
                </div>
              ) : (
                <p className="text-[11.5px] text-zinc-600">
                  Nenhuma restrição: todos os agentes leem e escrevem.
                </p>
              )}
            </>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}
