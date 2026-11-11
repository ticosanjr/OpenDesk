import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Loader2, Save, Terminal, Trash2 } from "lucide-react";
import { apiDelete, apiGet, apiPut } from "@/lib/api";
import type { CommandFile, ModelInfo, StudioScope } from "@/lib/types";
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

function blankCommand(scope: StudioScope): CommandFile {
  return {
    id: "",
    scope,
    path: "",
    description: "",
    agent: "",
    model: "",
    subagent: null,
    template: "",
    sha256: "",
  };
}

export function CommandPanel({
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

  const [commands, setCommands] = useState<CommandFile[]>([]);
  const [draft, setDraft] = useState<CommandFile | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const list = await apiGet<CommandFile[]>(`/api/studio/commands?${query}`);
      setCommands(list);
      setError(null);
      setDraft((current) => {
        if (!current || isNew) return current;
        return list.find((command) => command.id === current.id) ?? current;
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [query, isNew]);

  useEffect(() => {
    void load();
  }, [load, lastFsEvent?.path]);

  const lastFocus = useRef<string | null>(null);
  useEffect(() => {
    if (!focusId || lastFocus.current === focusId) return;
    const found = commands.find((command) => command.id === focusId);
    if (found) {
      lastFocus.current = focusId;
      setDraft({ ...found });
      setIsNew(false);
    }
  }, [focusId, commands]);

  useEffect(() => {
    apiGet<{ data: ModelInfo[] }>("/api/models")
      .then((response) => setModels(response.data))
      .catch(() => setModels([]));
  }, []);

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
      const saved = await apiPut<CommandFile>(
        `/api/studio/command?id=${encodeURIComponent(id)}&${query}`,
        {
          description: draft.description?.trim() || null,
          agent: draft.agent?.trim() || null,
          model: draft.model?.trim() || null,
          subagent: draft.subagent,
          template: draft.template,
          expected_sha256: isNew ? null : draft.sha256,
        },
      );
      setDraft(saved);
      setIsNew(false);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function remove() {
    if (!draft || isNew) return;
    if (!(await confirmAction({ title: `Excluir o comando /${draft.id}?`, description: "O arquivo .md será removido do escopo selecionado.", confirmLabel: "Excluir comando", tone: "destructive" }))) return;
    setSaving(true);
    try {
      await apiDelete(`/api/studio/command?id=${encodeURIComponent(draft.id)}&${query}`);
      setDraft(null);
      setIsNew(false);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <StudioList title="Comandos" count={commands.length} onNew={() => {
        setDraft(blankCommand(scope));
        setIsNew(true);
        setError(null);
      }}>
        {loading && commands.length === 0 ? (
          <p className="px-3 py-6 text-center text-[10px] text-zinc-500">Carregando…</p>
        ) : commands.length === 0 ? (
          <p className="px-3 py-6 text-center text-[10px] text-zinc-500">
            Nenhum comando neste escopo.
          </p>
        ) : (
          commands.map((command) => (
            <StudioListItem
              key={command.id}
              selected={draft?.id === command.id && !isNew}
              title={`/${command.id}`}
              subtitle={command.description ?? "sem descrição"}
              badge={<Badge variant="default">cmd</Badge>}
              onClick={() => {
                setDraft({ ...command });
                setIsNew(false);
                setError(null);
              }}
            />
          ))
        )}
      </StudioList>

      <div className="min-w-0 flex-1 overflow-y-auto p-5">
        <StudioError error={error} />
        {!draft ? (
          <StudioEmptyEditor
            icon={Terminal}
            title="Selecione um comando"
            description="Comandos são templates Markdown em .opencode/commands/<id>.md, invocados por /id no OpenCode."
          />
        ) : (
          <div className="mx-auto max-w-3xl space-y-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="flex items-center gap-2">
                  <h2 className="text-sm font-semibold text-zinc-100">
                    /{draft.id || "novo-comando"}
                  </h2>
                  {isNew ? <Badge variant="blue">novo</Badge> : null}
                </div>
                <p className="mt-0.5 font-mono text-[9px] text-zinc-500">
                  {draft.path || "arquivo será criado ao salvar"}
                </p>
              </div>
              <div className="flex gap-2">
                {!isNew ? (
                  <Button variant="destructive" size="sm" onClick={() => void remove()} disabled={saving}>
                    <Trash2 size={13} />
                    Excluir
                  </Button>
                ) : null}
                <Button size="sm" onClick={() => void save()} disabled={saving}>
                  {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
                  Salvar
                </Button>
              </div>
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <StudioField label="ID / slug" hint={isNew ? "Vira /id no OpenCode." : "O ID não é renomeado por esta tela."}>
                <input
                  value={draft.id}
                  disabled={!isNew}
                  onChange={(event) => setDraft({ ...draft, id: event.target.value })}
                  placeholder="auditar"
                  className={cn(inputClass, "font-mono disabled:opacity-60")}
                />
              </StudioField>
              <StudioField label="Descrição">
                <input
                  value={draft.description ?? ""}
                  onChange={(event) => setDraft({ ...draft, description: event.target.value })}
                  placeholder="Auditoria técnica rigorosa"
                  className={inputClass}
                />
              </StudioField>
              <StudioField label="Agente" hint="Opcional. Ex.: auditor, plan, build.">
                <input
                  value={draft.agent ?? ""}
                  onChange={(event) => setDraft({ ...draft, agent: event.target.value })}
                  placeholder="auditor"
                  className={cn(inputClass, "font-mono text-[11px]")}
                />
              </StudioField>
              <StudioField label="Modelo" hint="provider/model ou provider/model#variant.">
                <input
                  list="opendesk-command-models"
                  value={draft.model ?? ""}
                  onChange={(event) => setDraft({ ...draft, model: event.target.value })}
                  placeholder="opencode-go/glm-5.3"
                  className={cn(inputClass, "font-mono text-[11px]")}
                />
                <datalist id="opendesk-command-models">
                  {models.map((model) => {
                    const provider = model.providerID ?? "";
                    const id = model.modelID ?? model.id;
                    return <option key={`${provider}/${id}`} value={`${provider}/${id}`} />;
                  })}
                </datalist>
              </StudioField>
            </div>

            <StudioField label="Subagent">
              <select
                value={draft.subagent === null || draft.subagent === undefined ? "" : String(draft.subagent)}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    subagent: event.target.value === "" ? null : event.target.value === "true",
                  })
                }
                className={inputClass}
              >
                <option value="">herdar do agente</option>
                <option value="true">true — child session em background</option>
                <option value="false">false — sessão atual</option>
              </select>
            </StudioField>

            <StudioField label="Template" hint="Use $ARGUMENTS, $1, $2… e blocos !`comando` quando necessário.">
              <textarea
                value={draft.template}
                onChange={(event) => setDraft({ ...draft, template: event.target.value })}
                rows={14}
                className={cn(inputClass, "min-h-[260px] resize-y py-2 font-mono text-[11px] leading-relaxed")}
              />
            </StudioField>
          </div>
        )}
      </div>
    </div>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60 disabled:cursor-not-allowed";
