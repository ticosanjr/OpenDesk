import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Bot, Loader2, Save, Trash2 } from "lucide-react";
import { apiDelete, apiGet, apiPut } from "@/lib/api";
import type {
  AgentFile,
  ModelInfo,
  PermissionRule,
  StudioScope,
} from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  StudioEmptyEditor,
  StudioError,
  StudioField,
  StudioList,
  StudioListItem,
} from "@/components/studio/StudioList";
import { cn } from "@/lib/utils";
import { confirmAction } from "@/store/confirm-store";
import { AgentFormFields } from "./agent/AgentFormFields";
import { AgentPermissionsEditor } from "./agent/AgentPermissionsEditor";
import { blankAgent, initials, inputClass } from "./agent/agent-presets";

export function AgentPanel({
  scope,
  workspaceId,
  focusId,
}: {
  scope: StudioScope;
  workspaceId: string | null;
  focusId?: string | null;
}) {
  const lastFsEvent = useAppStore((state) => state.lastFsEvent);
  const query = useMemo(
    () => `scope=${scope}&workspace_id=${encodeURIComponent(workspaceId ?? "")}`,
    [scope, workspaceId],
  );

  const [agents, setAgents] = useState<AgentFile[]>([]);
  const [draft, setDraft] = useState<AgentFile | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [idTouched, setIdTouched] = useState(false);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Watcher events refresh the list only. The editor draft is owned by the
  // user: overwriting it here used to erase every keystroke while a run wrote
  // files elsewhere in the workspace.
  const load = useCallback(async () => {
    setLoading(true);
    try {
      const list = await apiGet<AgentFile[]>(`/api/studio/agents?${query}`);
      setAgents(list);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [query]);

  useEffect(() => {
    void load();
  }, [load, lastFsEvent?.path]);

  const lastFocus = useRef<string | null>(null);
  useEffect(() => {
    if (!focusId || lastFocus.current === focusId) return;
    const found = agents.find((agent) => agent.id === focusId);
    if (found) {
      lastFocus.current = focusId;
      setDraft({
        ...found,
        permissions: found.permissions.map((permission) => ({ ...permission })),
      });
      setIsNew(false);
    }
  }, [focusId, agents]);

  useEffect(() => {
    apiGet<{ data: ModelInfo[] }>("/api/models")
      .then((response) => setModels(response.data))
      .catch(() => setModels([]));
  }, []);

  function startNew() {
    setDraft(blankAgent(scope));
    setIsNew(true);
    setIdTouched(false);
    setError(null);
  }

  function select(agent: AgentFile) {
    setDraft({
      ...agent,
      permissions: agent.permissions.map((permission) => ({ ...permission })),
    });
    setIsNew(false);
    setError(null);
  }

  async function save() {
    if (!draft) return;
    const id = draft.id.trim();
    if (!/^[a-z0-9][a-z0-9-_/]*$/.test(id)) {
      setError("ID inválido. Use letras minúsculas, números, '-', '_' ou '/'.");
      return;
    }

    setSaving(true);
    setError(null);
    try {
      const saved = await apiPut<AgentFile>(
        `/api/studio/agent?id=${encodeURIComponent(id)}&${query}`,
        {
          name: draft.name?.trim() || null,
          avatar: draft.avatar?.trim() || null,
          description: draft.description?.trim() || null,
          mode: draft.mode || null,
          model: draft.model?.trim() || null,
          color: draft.color?.trim() || null,
          hidden: draft.hidden,
          steps: draft.steps ?? null,
          disabled: draft.disabled,
          permissions: draft.permissions.filter(
            (permission) => permission.action.trim() && permission.effect,
          ),
          system: draft.system,
          expected_sha256: isNew ? null : draft.sha256,
        },
      );
      setDraft(saved);
      setIsNew(false);
      await load();
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(
        message.includes("changed on disk")
          ? "Este agente mudou no disco. Feche e abra novamente para recarregar."
          : message,
      );
    } finally {
      setSaving(false);
    }
  }

  async function remove() {
    if (!draft || isNew) return;
    if (!(await confirmAction({ title: `Excluir o agente @${draft.id}?`, description: "O arquivo .md será removido do escopo selecionado.", confirmLabel: "Excluir agente", tone: "destructive" }))) return;
    setSaving(true);
    try {
      await apiDelete(`/api/studio/agent?id=${encodeURIComponent(draft.id)}&${query}`);
      setDraft(null);
      setIsNew(false);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  function updatePermission(index: number, patch: Partial<PermissionRule>) {
    if (!draft) return;
    const permissions = draft.permissions.map((permission, current) =>
      current === index ? { ...permission, ...patch } : permission,
    );
    setDraft({ ...draft, permissions });
  }

  function addPermission() {
    if (!draft) return;
    setDraft({
      ...draft,
      permissions: [
        ...draft.permissions,
        { action: "edit", resource: "*", effect: "deny" },
      ],
    });
  }

  function removePermission(index: number) {
    if (!draft) return;
    setDraft({
      ...draft,
      permissions: draft.permissions.filter((_, current) => current !== index),
    });
  }

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <StudioList
        title="Agentes"
        count={agents.length}
        onNew={startNew}
        newLabel="Novo agente"
      >
        {loading && agents.length === 0 ? (
          <p className="px-3 py-6 text-center text-[12px] text-zinc-500">Carregando…</p>
        ) : agents.length === 0 ? (
          <p className="px-3 py-6 text-center text-[12px] text-zinc-500">
            Nenhum agente neste escopo.
          </p>
        ) : (
          agents.map((agent) => (
            <StudioListItem
              key={`${agent.scope}:${agent.id}`}
              selected={draft?.id === agent.id && !isNew}
              title={agent.name || agent.id}
              subtitle={`@${agent.id} · ${agent.path.split("/").slice(-1)[0]}`}
              badge={
                <span
                  className="flex h-5 w-5 items-center justify-center rounded-full text-[11.5px]"
                  style={{ background: `${agent.color || "#3b82f6"}22`, color: agent.color || "#93c5fd" }}
                >
                  {initials(agent.avatar || agent.name || agent.id)}
                </span>
              }
              onClick={() => select(agent)}
            />
          ))
        )}
      </StudioList>

      <div className="min-w-0 flex-1 overflow-y-auto p-5">
        <StudioError error={error} />
        {!draft ? (
          <StudioEmptyEditor
            icon={Bot}
            title="Selecione um agente"
            description="Edite description, mode, model, permissões e system prompt. O arquivo .md é gravado diretamente em .opencode/agents/ ou no escopo global."
          />
        ) : (
          <div className="mx-auto max-w-3xl space-y-4">
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-center gap-3">
                <div
                  className="flex h-12 w-12 items-center justify-center rounded-xl border border-zinc-700/60 text-lg"
                  style={{
                    background: `${draft.color || "#60a5fa"}22`,
                    color: draft.color || "#93c5fd",
                  }}
                >
                  {initials(draft.avatar || draft.name || draft.id || "🤖")}
                </div>
                <div>
                  <div className="flex items-center gap-2">
                    <h2 className="text-sm font-semibold text-zinc-100">
                      {draft.name || draft.id || "Novo agente"}
                    </h2>
                    <Badge variant={draft.mode === "primary" ? "green" : "purple"}>
                      {draft.mode || "subagent"}
                    </Badge>
                    {isNew ? <Badge variant="blue">novo</Badge> : null}
                  </div>
                  <p className="mt-0.5 font-mono text-[11.5px] text-zinc-500">
                    {draft.path || "arquivo será criado ao salvar"}
                  </p>
                </div>
              </div>
              <div className="flex gap-2">
                {!isNew ? (
                  <Button
                    variant="destructive"
                    size="sm"
                    onClick={() => void remove()}
                    disabled={saving}
                  >
                    <Trash2 size={13} />
                    Excluir
                  </Button>
                ) : null}
                <Button size="sm" onClick={() => void save()} disabled={saving}>
                  {saving ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <Save size={14} />
                  )}
                  Salvar
                </Button>
              </div>
            </div>

            <AgentFormFields
              draft={draft}
              onDraftChange={setDraft}
              isNew={isNew}
              idTouched={idTouched}
              setIdTouched={setIdTouched}
              models={models}
            />

            <AgentPermissionsEditor
              permissions={draft.permissions}
              onAdd={addPermission}
              onUpdate={updatePermission}
              onRemove={removePermission}
            />

            <StudioField label="System prompt" hint="O corpo Markdown do arquivo é o system prompt do agente.">
              <textarea
                value={draft.system}
                onChange={(event) => setDraft({ ...draft, system: event.target.value })}
                rows={12}
                placeholder="Você é o Auditor de Qualidade do workspace…"
                className={cn(inputClass, "min-h-[220px] resize-y py-2 font-mono text-[13px] leading-relaxed")}
              />
            </StudioField>

            {!isNew ? (
              <p className="font-mono text-[11.5px] text-zinc-500">
                sha256 {draft.sha256.slice(0, 16)}…
              </p>
            ) : null}
          </div>
        )}
      </div>
    </div>
  );
}
