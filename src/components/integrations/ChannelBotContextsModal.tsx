import { useCallback, useEffect, useState } from "react";
import {
  AlertTriangle,
  Loader2,
  MessageSquarePlus,
  Plus,
  RotateCcw,
  Save,
  Trash2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { Switch } from "@/components/ui/switch";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import type {
  ChannelBot,
  ChannelChatContext,
  ChannelMemoryScope,
} from "@/lib/types";
import {
  SCOPE_HINT,
  SCOPE_LABEL,
} from "@/components/integrations/channel-context-scope";
import { cn } from "@/lib/utils";
import {
  smallInputClass as inputClass,
  smallLabelClass as labelClass,
} from "@/lib/ui-classes";

interface Draft {
  title: string;
  systemContext: string;
  agentOverride: string;
  memoryScope: ChannelMemoryScope;
  enabled: boolean;
}

function draftOf(context: ChannelChatContext): Draft {
  return {
    title: context.title,
    systemContext: context.system_context,
    agentOverride: context.agent_override ?? "",
    memoryScope: context.memory_scope,
    enabled: context.enabled,
  };
}

export function ChannelBotContextsModal({
  bot,
  open,
  onClose,
  onChanged,
}: {
  bot: ChannelBot | null;
  open: boolean;
  onClose: () => void;
  onChanged: () => void;
}) {
  const [contexts, setContexts] = useState<ChannelChatContext[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [newChat, setNewChat] = useState("");
  const [newTitle, setNewTitle] = useState("");

  const load = useCallback(async () => {
    if (!bot) return;
    setLoading(true);
    setError(null);
    try {
      const list = await apiGet<ChannelChatContext[]>(
        `/api/channels/bots/${bot.id}/contexts`,
      );
      setContexts(list);
      setSelectedId((current) => {
        const next = list.find((context) => context.id === current) ?? list[0];
        setDraft(next ? draftOf(next) : null);
        return next?.id ?? null;
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [bot]);

  useEffect(() => {
    setContexts([]);
    setSelectedId(null);
    setDraft(null);
    setAdding(false);
    setNewChat("");
    setNewTitle("");
    if (open) void load();
  }, [load, open]);

  function select(context: ChannelChatContext) {
    setSelectedId(context.id);
    setDraft(draftOf(context));
    setError(null);
  }

  async function saveDraft() {
    if (!bot || !selectedId || !draft || !draft.title.trim()) {
      setError("Informe um título para o grupo/chat.");
      return;
    }
    setBusy("save");
    setError(null);
    try {
      await apiPut(`/api/channels/bots/${bot.id}/contexts/${selectedId}`, {
        title: draft.title.trim(),
        system_context: draft.systemContext,
        agent_override: draft.agentOverride.trim() || null,
        memory_scope: draft.memoryScope,
        enabled: draft.enabled,
      });
      toast({ tone: "success", title: "Contexto do grupo salvo" });
      await load();
      onChanged();
    } catch (cause) {
      const description = cause instanceof Error ? cause.message : String(cause);
      setError(description);
      toast({ tone: "error", title: "Falha ao salvar o contexto", description });
    } finally {
      setBusy(null);
    }
  }

  async function toggleContext(context: ChannelChatContext) {
    if (!bot) return;
    setBusy(`toggle:${context.id}`);
    setError(null);
    try {
      await apiPut(`/api/channels/bots/${bot.id}/contexts/${context.id}`, {
        enabled: !context.enabled,
      });
      await load();
      onChanged();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function resetSession() {
    if (!bot || !selectedId) return;
    setBusy("reset");
    setError(null);
    try {
      await apiPut(`/api/channels/bots/${bot.id}/contexts/${selectedId}`, {
        opencode_session_id: null,
      });
      toast({
        tone: "success",
        title: "Sessão do grupo reiniciada",
        description: "A próxima mensagem abrirá uma nova sessão do OpenCode.",
      });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function createContext() {
    if (!bot) return;
    if (!newChat.trim()) {
      setError("Informe o ID do chat ou grupo.");
      return;
    }
    setBusy("create");
    setError(null);
    try {
      const created = await apiPost<ChannelChatContext>(
        `/api/channels/bots/${bot.id}/contexts`,
        {
          external_chat_id: newChat.trim(),
          title: newTitle.trim() || newChat.trim(),
          chat_type: "group",
          memory_scope: "chat",
        },
      );
      setAdding(false);
      setNewChat("");
      setNewTitle("");
      await load();
      setSelectedId(created.id);
      setDraft(draftOf(created));
      onChanged();
      toast({ tone: "success", title: "Grupo/chat adicionado" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function deleteContext() {
    if (!bot || !selectedId) return;
    const confirmed = await confirmAction({
      title: "Remover este contexto?",
      description:
        "O grupo deixa de ter instruções próprias e volta a usar o agente padrão do bot.",
      confirmLabel: "Remover contexto",
      tone: "destructive",
    });
    if (!confirmed) return;
    setBusy("delete");
    try {
      await apiDelete(`/api/channels/bots/${bot.id}/contexts/${selectedId}`);
      setSelectedId(null);
      setDraft(null);
      await load();
      onChanged();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  const selected = contexts.find((context) => context.id === selectedId) ?? null;

  return (
    <Modal
      open={open}
      onClose={onClose}
      size="xl"
      title={`Contextos de ${bot?.name ?? "bot"}`}
      description="Cada grupo ou conversa tem instruções, agente, escopo de memória e sessão próprios. Nada vaza entre grupos."
    >
      <div className="mt-4 grid gap-3 md:grid-cols-[240px,1fr]">
        <div className="space-y-2">
          <Button
            size="sm"
            variant="outline"
            className="w-full"
            onClick={() => setAdding((value) => !value)}
            disabled={busy !== null}
          >
            <Plus size={13} />
            Adicionar grupo/chat
          </Button>
          {adding ? (
            <div className="space-y-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-2.5">
              <label>
                <span className={labelClass}>ID do chat/grupo</span>
                <input
                  value={newChat}
                  onChange={(event) => setNewChat(event.target.value)}
                  className={cn(inputClass, "font-mono text-[12px]")}
                  placeholder="-1001234567890"
                />
              </label>
              <label>
                <span className={labelClass}>Título amigável</span>
                <input
                  value={newTitle}
                  onChange={(event) => setNewTitle(event.target.value)}
                  className={inputClass}
                  placeholder="Trabalho, Hobbies…"
                />
              </label>
              <div className="flex gap-1.5">
                <Button
                  size="sm"
                  onClick={() => void createContext()}
                  disabled={busy !== null}
                >
                  {busy === "create" ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <Plus size={12} />
                  )}
                  Adicionar
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setAdding(false)}
                >
                  Cancelar
                </Button>
              </div>
            </div>
          ) : null}

          {loading ? (
            <p className="flex items-center gap-2 px-1 text-[12px] text-zinc-400">
              <Loader2 size={13} className="animate-spin" />
              Carregando…
            </p>
          ) : null}

          {!loading && contexts.length === 0 ? (
            <p className="rounded-lg border border-dashed border-zinc-800 px-3 py-4 text-center text-[12px] text-zinc-500">
              Nenhum grupo detectado ainda. Envie uma mensagem para o bot ou
              adicione manualmente.
            </p>
          ) : null}

          <div className="space-y-1">
            {contexts.map((context) => (
              <button
                key={context.id}
                type="button"
                onClick={() => select(context)}
                className={cn(
                  "w-full rounded-lg border px-2.5 py-2 text-left transition-colors",
                  context.id === selectedId
                    ? "border-blue-500/40 bg-blue-500/10"
                    : "border-zinc-800/70 bg-zinc-950/30 hover:border-zinc-700",
                )}
              >
                <span className="flex items-center gap-1.5">
                  <span className="min-w-0 truncate text-[13px] font-medium text-zinc-200">
                    {context.title}
                  </span>
                  {!context.enabled ? (
                    <Badge variant="default">off</Badge>
                  ) : null}
                </span>
                <span className="mt-0.5 block truncate font-mono text-[11px] text-zinc-500">
                  {context.external_chat_id}
                </span>
                <span className="mt-0.5 block text-[11px] text-zinc-500">
                  memória: {SCOPE_LABEL[context.memory_scope]}
                </span>
              </button>
            ))}
          </div>
        </div>

        <div className="min-w-0 rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
          {!selected || !draft ? (
            <div className="flex h-full min-h-40 flex-col items-center justify-center text-center">
              <MessageSquarePlus size={22} className="text-zinc-600" />
              <p className="mt-2 text-[13px] font-medium text-zinc-300">
                Selecione um grupo
              </p>
              <p className="mt-1 max-w-xs text-[12px] text-zinc-500">
                Ou adicione manualmente um chat para definir instruções
                exclusivas.
              </p>
            </div>
          ) : (
            <div className="space-y-2.5">
              <div className="grid gap-2 md:grid-cols-2">
                <label>
                  <span className={labelClass}>Título</span>
                  <input
                    value={draft.title}
                    onChange={(event) =>
                      setDraft({ ...draft, title: event.target.value })
                    }
                    className={inputClass}
                  />
                </label>
                <label>
                  <span className={labelClass}>Agente do grupo (opcional)</span>
                  <input
                    value={draft.agentOverride}
                    onChange={(event) =>
                      setDraft({ ...draft, agentOverride: event.target.value })
                    }
                    className={inputClass}
                    placeholder="usa o agente padrão do bot"
                  />
                </label>
              </div>
              <label className="block">
                <span className={labelClass}>Instruções exclusivas do grupo</span>
                <textarea
                  value={draft.systemContext}
                  onChange={(event) =>
                    setDraft({ ...draft, systemContext: event.target.value })
                  }
                  rows={5}
                  className={cn(inputClass, "h-auto min-h-28 resize-y py-2")}
                  placeholder="Ex.: responda como o time de engenharia; seja breve; não use memória pessoal."
                />
              </label>
              <div className="grid gap-2 md:grid-cols-2">
                <label>
                  <span className={labelClass}>Escopo de memória</span>
                  <select
                    value={draft.memoryScope}
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        memoryScope: event.target.value as ChannelMemoryScope,
                      })
                    }
                    className={inputClass}
                  >
                    {(
                      ["chat", "normal", "both", "none"] as ChannelMemoryScope[]
                    ).map((scope) => (
                      <option key={scope} value={scope}>
                        {SCOPE_LABEL[scope]}
                      </option>
                    ))}
                  </select>
                  <span className="mt-1 block text-[11px] text-zinc-500">
                    {SCOPE_HINT[draft.memoryScope]}
                  </span>
                </label>
                <div className="flex flex-col justify-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
                  <label className="flex items-center gap-2 text-[12.5px] text-zinc-300">
                    <Switch
                      checked={draft.enabled}
                      onCheckedChange={(value) =>
                        setDraft({ ...draft, enabled: value })
                      }
                      label="Contexto ativo"
                    />
                    Contexto ativo
                  </label>
                  <p className="text-[11px] text-zinc-500">
                    {selected.opencode_session_id
                      ? `Sessão isolada: ${selected.opencode_session_id.slice(0, 12)}…`
                      : "Sem sessão aberta para este grupo."}
                  </p>
                </div>
              </div>

              <div className="flex flex-wrap gap-1.5 border-t border-zinc-800/70 pt-2.5">
                <Button
                  size="sm"
                  onClick={() => void saveDraft()}
                  disabled={busy !== null}
                >
                  {busy === "save" ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <Save size={12} />
                  )}
                  Salvar
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void resetSession()}
                  disabled={busy !== null || !selected.opencode_session_id}
                >
                  {busy === "reset" ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <RotateCcw size={12} />
                  )}
                  Reiniciar sessão do grupo
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void toggleContext(selected)}
                  disabled={busy !== null}
                >
                  {busy === `toggle:${selected.id}` ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : null}
                  {selected.enabled ? "Desativar" : "Ativar"}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void deleteContext()}
                  disabled={busy !== null}
                >
                  {busy === "delete" ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <Trash2 size={12} />
                  )}
                  Remover
                </Button>
              </div>
            </div>
          )}
          {error ? (
            <p className="mt-2 flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
              <AlertTriangle size={12} className="mt-0.5 shrink-0" />
              {error}
            </p>
          ) : null}
        </div>
      </div>
    </Modal>
  );
}
