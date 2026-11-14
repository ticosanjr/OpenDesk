import { useCallback, useEffect, useState } from "react";
import { Brain, Loader2, Pencil, Plus, Save, Trash2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import type { PersonalContextGroup } from "@/lib/types";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { useAppStore } from "@/store/app-store";
import { cn } from "@/lib/utils";
import {
  smallInputClass as inputClass,
  smallLabelClass as labelClass,
} from "@/lib/ui-classes";

export function PersonalContextCard() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [groups, setGroups] = useState<PersonalContextGroup[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState({
    name: "",
    instructions: "",
    scope: "workspace" as "workspace" | "global",
  });
  const [editing, setEditing] = useState<string | null>(null);
  const [editDraft, setEditDraft] = useState({
    name: "",
    instructions: "",
    scope: "workspace" as "workspace" | "global",
  });

  const query = workspaceId
    ? `?workspace_id=${encodeURIComponent(workspaceId)}`
    : "";

  const load = useCallback(async () => {
    if (!workspaceId) {
      setGroups([]);
      return;
    }
    setLoading(true);
    try {
      const value = await apiGet<PersonalContextGroup[]>(
        `/api/personal-context${query}`,
      );
      setGroups(Array.isArray(value) ? value : []);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [query, workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function create() {
    if (!draft.name.trim() || !draft.instructions.trim()) {
      setError("Informe um nome e as instruções.");
      return;
    }
    setBusy("create");
    try {
      await apiPost(`/api/personal-context${query}`, {
        scope: draft.scope,
        workspace_id: draft.scope === "workspace" ? workspaceId : null,
        name: draft.name.trim(),
        instructions: draft.instructions.trim(),
        is_enabled: true,
      });
      setDraft({ name: "", instructions: "", scope: "workspace" });
      setCreating(false);
      await load();
      toast({ tone: "success", title: "Contexto pessoal criado" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  function startEdit(group: PersonalContextGroup) {
    setEditing(group.id);
    setEditDraft({
      name: group.name,
      instructions: group.instructions,
      scope: group.scope,
    });
  }

  async function saveEdit(group: PersonalContextGroup) {
    if (!editDraft.name.trim() || !editDraft.instructions.trim()) {
      setError("Informe um nome e as instruções.");
      return;
    }
    setBusy(group.id);
    try {
      await apiPut(`/api/personal-context/${group.id}`, {
        scope: editDraft.scope,
        workspace_id:
          editDraft.scope === "workspace"
            ? (group.workspace_id ?? workspaceId)
            : null,
        name: editDraft.name.trim(),
        instructions: editDraft.instructions.trim(),
        is_enabled: group.is_enabled,
        position: group.position,
      });
      setEditing(null);
      await load();
      toast({ tone: "success", title: "Contexto atualizado" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function toggle(group: PersonalContextGroup) {
    setBusy(group.id);
    try {
      await apiPut(`/api/personal-context/${group.id}`, {
        scope: group.scope,
        workspace_id: group.workspace_id,
        name: group.name,
        instructions: group.instructions,
        is_enabled: !group.is_enabled,
        position: group.position,
      });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function remove(group: PersonalContextGroup) {
    if (
      !(await confirmAction({
        title: `Remover “${group.name}”?`,
        description:
          group.scope === "global"
            ? "Este grupo global deixa de entrar no prompt de todos os workspaces."
            : "Este grupo deixa de entrar no prompt deste workspace.",
        confirmLabel: "Remover contexto",
        tone: "destructive",
      }))
    ) {
      return;
    }
    setBusy(group.id);
    try {
      await apiDelete(`/api/personal-context/${group.id}`);
      await load();
      toast({ tone: "success", title: "Contexto removido" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Card>
      <CardHeader className="py-3">
        <div>
          <CardTitle className="flex items-center gap-2">
            <Brain size={15} className="text-violet-300" />
            Contexto pessoal
          </CardTitle>
          <p className="mt-0.5 text-[12px] text-zinc-500">
            Instruções curtas que entram automaticamente nos prompts. Grupos
            globais valem para todos os workspaces.
          </p>
        </div>
        <Button
          size="sm"
          variant="outline"
          onClick={() => setCreating((value) => !value)}
        >
          <Plus size={12} />
          Novo grupo
        </Button>
      </CardHeader>
      <CardContent className="space-y-3">
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            {error}
          </p>
        ) : null}

        {creating ? (
          <div className="space-y-2 rounded-lg border border-violet-500/20 bg-violet-500/[0.03] p-3">
            <div className="grid gap-2 sm:grid-cols-[1fr_150px]">
              <label>
                <span className={labelClass}>Nome do grupo</span>
                <input
                  value={draft.name}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      name: event.target.value,
                    }))
                  }
                  placeholder="Ex.: tom de voz"
                  className={inputClass}
                />
              </label>
              <label>
                <span className={labelClass}>Escopo</span>
                <select
                  value={draft.scope}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      scope: event.target.value as "workspace" | "global",
                    }))
                  }
                  className={inputClass}
                >
                  <option value="workspace">workspace</option>
                  <option value="global">global</option>
                </select>
              </label>
            </div>
            <label>
              <span className={labelClass}>Instruções</span>
              <textarea
                value={draft.instructions}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    instructions: event.target.value,
                  }))
                }
                rows={3}
                placeholder="Ex.: responda em português, seja direto e cite fontes."
                className={cn(inputClass, "h-auto resize-y py-2")}
              />
            </label>
            <div className="flex justify-end gap-1.5">
              <Button size="sm" variant="ghost" onClick={() => setCreating(false)}>
                <X size={12} />
                Cancelar
              </Button>
              <Button
                size="sm"
                onClick={() => void create()}
                disabled={busy === "create"}
              >
                {busy === "create" ? (
                  <Loader2 size={12} className="animate-spin" />
                ) : (
                  <Save size={12} />
                )}
                Adicionar
              </Button>
            </div>
          </div>
        ) : null}

        {loading && groups.length === 0 ? (
          <p className="flex items-center gap-2 text-[12px] text-zinc-500">
            <Loader2 size={12} className="animate-spin" />
            Carregando contexto…
          </p>
        ) : groups.length === 0 ? (
          <p className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-3 text-[12.5px] text-zinc-500">
            Nenhum contexto pessoal ainda. Crie grupos para orientar todos os
            prompts sem repetir as mesmas instruções.
          </p>
        ) : (
          <div className="space-y-2">
            {groups.map((group) => {
              const isEditing = editing === group.id;
              return (
                <div
                  key={group.id}
                  className={cn(
                    "rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3",
                    !group.is_enabled && "opacity-60",
                  )}
                >
                  <div className="flex flex-wrap items-start gap-2">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <p className="truncate text-[13.5px] font-medium text-zinc-200">
                          {group.name}
                        </p>
                        <Badge
                          variant={group.scope === "global" ? "purple" : "default"}
                        >
                          {group.scope}
                        </Badge>
                      </div>
                      {!isEditing ? (
                        <p className="mt-1 whitespace-pre-wrap text-[12.5px] leading-relaxed text-zinc-400">
                          {group.instructions}
                        </p>
                      ) : null}
                    </div>
                    <div className="flex items-center gap-1">
                      <Switch
                        checked={group.is_enabled}
                        onCheckedChange={() => void toggle(group)}
                        disabled={busy === group.id}
                        label={
                          group.is_enabled
                            ? "Desativar contexto"
                            : "Ativar contexto"
                        }
                      />
                      {!isEditing ? (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => startEdit(group)}
                          disabled={busy === group.id}
                        >
                          <Pencil size={12} />
                        </Button>
                      ) : null}
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-red-300/80"
                        onClick={() => void remove(group)}
                        disabled={busy === group.id}
                      >
                        {busy === group.id ? (
                          <Loader2 size={12} className="animate-spin" />
                        ) : (
                          <Trash2 size={12} />
                        )}
                      </Button>
                    </div>
                  </div>
                  {isEditing ? (
                    <div className="mt-2 space-y-2">
                      <div className="grid gap-2 sm:grid-cols-[1fr_150px]">
                        <input
                          value={editDraft.name}
                          onChange={(event) =>
                            setEditDraft((current) => ({
                              ...current,
                              name: event.target.value,
                            }))
                          }
                          className={inputClass}
                          aria-label="Nome do grupo"
                        />
                        <select
                          value={editDraft.scope}
                          onChange={(event) =>
                            setEditDraft((current) => ({
                              ...current,
                              scope: event.target.value as "workspace" | "global",
                            }))
                          }
                          className={inputClass}
                          aria-label="Escopo do grupo"
                        >
                          <option value="workspace">workspace</option>
                          <option value="global">global</option>
                        </select>
                      </div>
                      <textarea
                        value={editDraft.instructions}
                        onChange={(event) =>
                          setEditDraft((current) => ({
                            ...current,
                            instructions: event.target.value,
                          }))
                        }
                        rows={3}
                        className={cn(inputClass, "h-auto resize-y py-2")}
                        aria-label="Instruções do contexto"
                      />
                      <div className="flex justify-end gap-1.5">
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => setEditing(null)}
                        >
                          Cancelar
                        </Button>
                        <Button
                          size="sm"
                          onClick={() => void saveEdit(group)}
                          disabled={busy === group.id}
                        >
                          {busy === group.id ? (
                            <Loader2 size={12} className="animate-spin" />
                          ) : (
                            <Save size={12} />
                          )}
                          Salvar
                        </Button>
                      </div>
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
