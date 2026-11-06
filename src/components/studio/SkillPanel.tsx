import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { BookOpen, Loader2, Plus, Save, Trash2 } from "lucide-react";
import { apiDelete, apiGet, apiPut } from "@/lib/api";
import type { SkillFile, StudioScope } from "@/lib/types";
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

function blankSkill(scope: StudioScope): SkillFile {
  return {
    id: "",
    scope,
    path: "",
    name: "",
    description: "",
    body: "",
    files: [],
    sha256: "",
  };
}

export function SkillPanel({
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

  const [skills, setSkills] = useState<SkillFile[]>([]);
  const [draft, setDraft] = useState<SkillFile | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const list = await apiGet<SkillFile[]>(`/api/studio/skills?${query}`);
      setSkills(list);
      setError(null);
      setDraft((current) => {
        if (!current || isNew) return current;
        return list.find((skill) => skill.id === current.id) ?? current;
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
    const found = skills.find((skill) => skill.id === focusId);
    if (found) {
      lastFocus.current = focusId;
      setDraft({ ...found, files: found.files.map((file) => ({ ...file })) });
      setIsNew(false);
    }
  }, [focusId, skills]);

  async function save() {
    if (!draft) return;
    const id = draft.id.trim();
    if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(id)) {
      setError("ID inválido. Use kebab-case, por exemplo: relatorio-executivo.");
      return;
    }

    setSaving(true);
    setError(null);
    try {
      const saved = await apiPut<SkillFile>(
        `/api/studio/skill?id=${encodeURIComponent(id)}&${query}`,
        {
          name: draft.name?.trim() || null,
          description: draft.description?.trim() || null,
          body: draft.body,
          files: draft.files,
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
    if (
      !(await confirmAction({
        title: `Excluir a skill ${draft.id}?`,
        description: "A pasta inteira da skill será removida do escopo selecionado.",
        confirmLabel: "Excluir skill",
        tone: "destructive",
      }))
    ) {
      return;
    }
    setSaving(true);
    try {
      await apiDelete(`/api/studio/skill?id=${encodeURIComponent(draft.id)}&${query}`);
      setDraft(null);
      setIsNew(false);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  function addFile() {
    if (!draft) return;
    setDraft({ ...draft, files: [...draft.files, { path: "", content: "" }] });
  }

  function updateFile(index: number, patch: Partial<{ path: string; content: string }>) {
    if (!draft) return;
    setDraft({
      ...draft,
      files: draft.files.map((file, current) =>
        current === index ? { ...file, ...patch } : file,
      ),
    });
  }

  function removeFile(index: number) {
    if (!draft) return;
    setDraft({ ...draft, files: draft.files.filter((_, current) => current !== index) });
  }

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <StudioList title="Skills" count={skills.length} onNew={() => {
        setDraft(blankSkill(scope));
        setIsNew(true);
        setError(null);
      }}>
        {loading && skills.length === 0 ? (
          <p className="px-3 py-6 text-center text-[10px] text-zinc-600">Carregando…</p>
        ) : skills.length === 0 ? (
          <p className="px-3 py-6 text-center text-[10px] text-zinc-600">
            Nenhuma skill neste escopo.
          </p>
        ) : (
          skills.map((skill) => (
            <StudioListItem
              key={skill.id}
              selected={draft?.id === skill.id && !isNew}
              title={skill.name || skill.id}
              subtitle={`${skill.id} · ${skill.files.length} arquivo(s)`}
              badge={<Badge variant="default">skill</Badge>}
              onClick={() => {
                setDraft({ ...skill, files: skill.files.map((file) => ({ ...file })) });
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
            icon={BookOpen}
            title="Selecione uma skill"
            description="Cada skill é uma pasta em .opencode/skills/<id>/ com SKILL.md e arquivos de referência/scripts."
          />
        ) : (
          <div className="mx-auto max-w-3xl space-y-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="flex items-center gap-2">
                  <h2 className="text-sm font-semibold text-zinc-100">
                    {draft.name || draft.id || "Nova skill"}
                  </h2>
                  {isNew ? <Badge variant="blue">nova</Badge> : null}
                </div>
                <p className="mt-0.5 font-mono text-[9px] text-zinc-600">
                  {draft.path || "pasta será criada ao salvar"}
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
              <StudioField label="ID / slug" hint={isNew ? "Vira o nome da pasta." : "O ID não é renomeado por esta tela."}>
                <input
                  value={draft.id}
                  disabled={!isNew}
                  onChange={(event) => setDraft({ ...draft, id: event.target.value })}
                  placeholder="relatorio-executivo"
                  className={cn(inputClass, "font-mono disabled:opacity-60")}
                />
              </StudioField>
              <StudioField label="Nome de exibição">
                <input
                  value={draft.name ?? ""}
                  onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                  placeholder="Relatório Executivo"
                  className={inputClass}
                />
              </StudioField>
            </div>

            <StudioField label="Descrição">
              <input
                value={draft.description ?? ""}
                onChange={(event) => setDraft({ ...draft, description: event.target.value })}
                placeholder="Padroniza relatórios executivos"
                className={inputClass}
              />
            </StudioField>

            <StudioField label="Instruções (SKILL.md)">
              <textarea
                value={draft.body}
                onChange={(event) => setDraft({ ...draft, body: event.target.value })}
                rows={12}
                className={cn(inputClass, "min-h-[240px] resize-y py-2 font-mono text-[11px] leading-relaxed")}
              />
            </StudioField>

            <div className="rounded-xl border border-zinc-800/80 bg-zinc-900/25">
              <div className="flex items-center justify-between border-b border-zinc-800/60 px-3 py-2">
                <span className="text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500">
                  Arquivos auxiliares
                </span>
                <Button variant="outline" size="sm" className="h-7" onClick={addFile}>
                  <Plus size={12} />
                  Adicionar
                </Button>
              </div>
              <div className="space-y-3 p-3">
                {draft.files.length === 0 ? (
                  <p className="text-[10px] text-zinc-600">
                    Nenhum arquivo auxiliar. Ex.: references/policy.md ou scripts/changelog.ts.
                  </p>
                ) : (
                  draft.files.map((file, index) => (
                    <div key={index} className="space-y-1.5 rounded-lg border border-zinc-800/70 p-2.5">
                      <div className="flex gap-2">
                        <input
                          value={file.path}
                          onChange={(event) => updateFile(index, { path: event.target.value })}
                          placeholder="references/policy.md"
                          className={cn(inputClass, "font-mono text-[10px]")}
                        />
                        <button
                          type="button"
                          onClick={() => removeFile(index)}
                          className="rounded-lg px-2 text-zinc-600 hover:bg-zinc-800 hover:text-red-300"
                          title="Remover arquivo"
                        >
                          <Trash2 size={13} />
                        </button>
                      </div>
                      <textarea
                        value={file.content}
                        onChange={(event) => updateFile(index, { content: event.target.value })}
                        rows={5}
                        className={cn(inputClass, "resize-y py-2 font-mono text-[10px]")}
                      />
                    </div>
                  ))
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60 disabled:cursor-not-allowed";
