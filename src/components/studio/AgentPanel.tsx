import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Bot, Loader2, Save, Trash2 } from "lucide-react";
import { apiDelete, apiGet, apiPut } from "@/lib/api";
import { slugifyWorkspace } from "@/lib/folder";
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

const ACTIONS = [
  "*",
  "edit",
  "shell",
  "read",
  "glob",
  "grep",
  "webfetch",
  "websearch",
  "subagent",
  "skill",
];

const EFFECTS = ["allow", "ask", "deny"];

const AVATAR_PRESETS = [
  "🤖",
  "🧠",
  "🛡️",
  "🧾",
  "📊",
  "✍️",
  "🔍",
  "⚙️",
  "🚀",
  "💡",
  "🎯",
  "📚",
  "🦉",
  "🧩",
  "🕵️",
  "🧑‍💻",
  "📈",
  "🧪",
  "🗂️",
  "🔧",
];

const COLOR_PRESETS = [
  "#60a5fa",
  "#34d399",
  "#fbbf24",
  "#f87171",
  "#a78bfa",
  "#22d3ee",
  "#fb923c",
  "#f472b6",
];

function blankAgent(scope: StudioScope): AgentFile {
  return {
    id: "",
    scope,
    path: "",
    name: "",
    avatar: "🤖",
    description: "",
    mode: "subagent",
    model: "",
    color: "#60a5fa",
    router_policy: null,
    hidden: false,
    steps: null,
    disabled: false,
    permissions: [],
    system: "",
    sha256: "",
  };
}

function initials(value: string): string {
  const chars = Array.from(value.trim());
  return chars.slice(0, 2).join("") || "?";
}

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

            <div className="grid gap-3 md:grid-cols-2">
              <StudioField label="Nome de exibição" hint="Usado pelo OpenDesk para identificar o agente; o OpenCode ignora este campo.">
                <input
                  value={draft.name ?? ""}
                  onChange={(event) => {
                    const name = event.target.value;
                    const next = { ...draft, name };
                    if (isNew && !idTouched) {
                      next.id = slugifyWorkspace(name);
                    }
                    setDraft(next);
                  }}
                  placeholder="Auditor de Qualidade"
                  className={inputClass}
                />
              </StudioField>
              <StudioField
                label="ID / slug"
                hint={
                  isNew
                    ? "Gerado automaticamente do nome. Edite apenas se precisar."
                    : "O ID não é renomeado por esta tela."
                }
              >
                <input
                  value={draft.id}
                  disabled={!isNew}
                  onChange={(event) => {
                    setIdTouched(true);
                    setDraft({ ...draft, id: event.target.value });
                  }}
                  placeholder="auditor-de-qualidade"
                  className={cn(inputClass, "font-mono disabled:opacity-60")}
                />
              </StudioField>
              <StudioField label="Avatar" hint="Escolha um preset ou digite outro emoji.">
                <div className="space-y-2">
                  <div className="grid grid-cols-10 gap-1">
                    {AVATAR_PRESETS.map((preset) => (
                      <button
                        key={preset}
                        type="button"
                        title={preset}
                        onClick={() => setDraft({ ...draft, avatar: preset })}
                        className={cn(
                          "flex h-7 items-center justify-center rounded-md border text-[14px] transition",
                          draft.avatar === preset
                            ? "workspace-selected"
                            : "border-zinc-800 bg-zinc-900/40 hover:border-zinc-600",
                        )}
                      >
                        {preset}
                      </button>
                    ))}
                  </div>
                  <div className="flex gap-2">
                    <input
                      value={draft.avatar ?? ""}
                      onChange={(event) =>
                        setDraft({ ...draft, avatar: event.target.value })
                      }
                      placeholder="🛡️"
                      maxLength={8}
                      className={cn(inputClass, "flex-1")}
                    />
                    <span className="flex h-9 w-9 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50 text-lg">
                      {draft.avatar || "🤖"}
                    </span>
                  </div>
                </div>
              </StudioField>
              <StudioField label="Cor" hint="Presets ou cor personalizada.">
                <div className="space-y-2">
                  <div className="flex flex-wrap gap-1.5">
                    {COLOR_PRESETS.map((preset) => (
                      <button
                        key={preset}
                        type="button"
                        title={preset}
                        onClick={() => setDraft({ ...draft, color: preset })}
                        className={cn(
                          "h-6 w-6 rounded-full border-2 transition",
                          (draft.color || "#60a5fa") === preset
                            ? "border-white/80"
                            : "border-transparent hover:border-zinc-500",
                        )}
                        style={{ background: preset }}
                      />
                    ))}
                  </div>
                  <div className="flex h-9 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2">
                    <input
                      type="color"
                      value={draft.color || "#60a5fa"}
                      onChange={(event) =>
                        setDraft({ ...draft, color: event.target.value })
                      }
                      className="h-5 w-8 cursor-pointer rounded border-0 bg-transparent p-0"
                    />
                    <span className="font-mono text-[12px] text-zinc-500">
                      {draft.color || "#60a5fa"}
                    </span>
                  </div>
                </div>
              </StudioField>
              <StudioField
                label="Política de roteador padrão"
                hint="Usada como default em rotinas que escolherem este agente."
              >
                <select
                  value={draft.router_policy ?? ""}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      router_policy: event.target.value || null,
                    })
                  }
                  className={inputClass}
                >
                  <option value="">herdar do workspace</option>
                  <option value="COST_BENEFIT">Custo-benefício</option>
                  <option value="CHEAPEST">Mais barato</option>
                  <option value="BEST_QUALITY">Máxima qualidade</option>
                  <option value="MANUAL">Manual</option>
                  <option value="CUSTOM_RULES">Regras customizadas</option>
                </select>
              </StudioField>
              <StudioField label="Modo">
                <select
                  value={draft.mode ?? "subagent"}
                  onChange={(event) => setDraft({ ...draft, mode: event.target.value })}
                  className={inputClass}
                >
                  <option value="primary">primary</option>
                  <option value="subagent">subagent</option>
                  <option value="all">all</option>
                </select>
              </StudioField>
              <StudioField label="Modelo" hint="provider/model ou provider/model#variant.">
                <input
                  list="opendesk-models"
                  value={draft.model ?? ""}
                  onChange={(event) => setDraft({ ...draft, model: event.target.value })}
                  placeholder="opencode-go/glm-5.3"
                  className={cn(inputClass, "font-mono text-[13px]")}
                />
                <datalist id="opendesk-models">
                  {models.map((model) => {
                    const provider = model.providerID ?? "";
                    const id = model.modelID ?? model.id;
                    return (
                      <option
                        key={`${provider}/${id}`}
                        value={`${provider}/${id}`}
                      />
                    );
                  })}
                </datalist>
              </StudioField>
              <StudioField label="Steps" hint="Máximo de passos do modelo; vazio usa o padrão.">
                <input
                  type="number"
                  min={1}
                  value={draft.steps ?? ""}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      steps: event.target.value ? Number(event.target.value) : null,
                    })
                  }
                  className={inputClass}
                />
              </StudioField>
              <div className="flex items-end gap-4 pb-1">
                <label className="flex items-center gap-2 text-[13px] text-zinc-400">
                  <input
                    type="checkbox"
                    checked={draft.hidden}
                    onChange={(event) => setDraft({ ...draft, hidden: event.target.checked })}
                    className="accent-blue-500"
                  />
                  hidden
                </label>
                <label className="flex items-center gap-2 text-[13px] text-zinc-400">
                  <input
                    type="checkbox"
                    checked={draft.disabled}
                    onChange={(event) => setDraft({ ...draft, disabled: event.target.checked })}
                    className="accent-blue-500"
                  />
                  disabled
                </label>
              </div>
            </div>

            <StudioField label="Descrição">
              <input
                value={draft.description ?? ""}
                onChange={(event) => setDraft({ ...draft, description: event.target.value })}
                placeholder="Especialista em auditoria de qualidade"
                className={inputClass}
              />
            </StudioField>

            <div className="rounded-xl border border-zinc-800/80 bg-zinc-900/25">
              <div className="flex items-center justify-between border-b border-zinc-800/60 px-3 py-2">
                <span className="text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
                  Permissões V2
                </span>
                <Button variant="outline" size="sm" className="h-7" onClick={addPermission}>
                  Adicionar regra
                </Button>
              </div>
              <div className="space-y-2 p-3">
                {draft.permissions.length === 0 ? (
                  <p className="text-[12px] text-zinc-500">
                    Sem regras próprias. O agente herda as permissões globais do OpenCode.
                  </p>
                ) : (
                  draft.permissions.map((permission, index) => (
                    <div key={index} className="grid grid-cols-[130px_1fr_100px_28px] gap-2">
                      <select
                        value={permission.action}
                        onChange={(event) =>
                          updatePermission(index, { action: event.target.value })
                        }
                        className={cn(inputClass, "font-mono text-[12px]")}
                      >
                        {ACTIONS.map((action) => (
                          <option key={action} value={action}>
                            {action}
                          </option>
                        ))}
                      </select>
                      <input
                        value={permission.resource}
                        onChange={(event) =>
                          updatePermission(index, { resource: event.target.value })
                        }
                        placeholder="*"
                        className={cn(inputClass, "font-mono text-[12px]")}
                      />
                      <select
                        value={permission.effect}
                        onChange={(event) =>
                          updatePermission(index, { effect: event.target.value })
                        }
                        className={cn(inputClass, "font-mono text-[12px]")}
                      >
                        {EFFECTS.map((effect) => (
                          <option key={effect} value={effect}>
                            {effect}
                          </option>
                        ))}
                      </select>
                      <button
                        type="button"
                        onClick={() => removePermission(index)}
                        className="rounded-lg text-zinc-500 hover:bg-zinc-800 hover:text-red-300"
                        title="Remover"
                      >
                        <Trash2 size={13} className="mx-auto" />
                      </button>
                    </div>
                  ))
                )}
              </div>
            </div>

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

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[13.5px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60 disabled:cursor-not-allowed";
