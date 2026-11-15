import { useCallback, useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  ArrowDown,
  Bot,
  Check,
  Paperclip,
  Loader2,
  MessageSquare,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Send,
  Square,
  Sparkles,
  Trash2,
  Wand2,
  X,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
import type { ChatMessage, ChatReply, ChatThread, ChatTurn } from "@/lib/types";
import { cn } from "@/lib/utils";
import { relativeTime } from "@/lib/format";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { toast } from "@/store/toast-store";
import { links } from "@/lib/links";
import { EntityLink } from "@/components/ui/entity-link";
import { confirmAction } from "@/store/confirm-store";
import { smallLabelClass as labelClass } from "@/lib/ui-classes";
import { useStickToBottom } from "@/lib/use-stick-to-bottom";

export function ChatPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );

  const [threads, setThreads] = useState<ChatThread[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [thread, setThread] = useState<ChatThread | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [agent, setAgent] = useState("build");
  const [model, setModel] = useState<string | null>(null);
  const [attachments, setAttachments] = useState<Array<{ name: string; data: string }>>([]);
  const [prompt, setPrompt] = useState("");
  const [loading, setLoading] = useState(false);
  const [sending, setSending] = useState(false);
  const [latestTurn, setLatestTurn] = useState<ChatTurn | null>(null);
  const [stopping, setStopping] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [threadQuery, setThreadQuery] = useState("");
  const [renaming, setRenaming] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [params, setParams] = useSearchParams();
  const lastChatEvent = useAppStore((state) => state.lastChatEvent);

  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const filteredThreads = threads.filter((item) => {
    const needle = threadQuery.trim().toLowerCase();
    if (!needle) return true;
    return `${item.title} ${item.agent}`.toLowerCase().includes(needle);
  });

  const loadThreads = useCallback(async () => {
    if (!activeWorkspaceId) {
      setThreads([]);
      return;
    }
    setLoading(true);
    try {
      const list = await apiGet<ChatThread[]>(
        `/api/chat/threads?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
      );
      setThreads(list);
      setSelectedId((current) =>
        current && list.some((item) => item.id === current)
          ? current
          : list[0]?.id ?? null,
      );
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId]);

  useEffect(() => {
    void loadThreads();
  }, [loadThreads]);

  useEffect(() => {
    const requested = params.get("thread");
    if (!requested || threads.length === 0) return;
    if (threads.some((item) => item.id === requested)) {
      setSelectedId(requested);
      setParams({}, { replace: true });
    }
  }, [params, setParams, threads]);

  useEffect(() => {
    if (!selectedId) {
      setThread(null);
      setMessages([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    apiGet<{ thread: ChatThread; messages: ChatMessage[] }>(
      `/api/chat/threads/${selectedId}`,
    )
      .then((value) => {
        if (cancelled) return;
        setThread(value.thread);
        setMessages(value.messages);
        setAgent(value.thread.agent);
        setModel(value.thread.model ?? null);
      })
      .catch((cause) =>
        setError(cause instanceof Error ? cause.message : String(cause)),
      )
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  useEffect(() => {
    if (!selectedId) {
      setLatestTurn(null);
      return;
    }
    let cancelled = false;
    void apiGet<ChatTurn | null>(
      `/api/chat/threads/${encodeURIComponent(selectedId)}/turns/latest`,
    )
      .then((value) => {
        if (!cancelled) setLatestTurn(value);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  const turnRunning = latestTurn?.status === "RUNNING";
  const thinking = sending || turnRunning;

  useEffect(() => {
    if (!selectedId || (!sending && !turnRunning)) return;
    const interval = window.setInterval(() => {
      void apiGet<ChatTurn | null>(
        `/api/chat/threads/${encodeURIComponent(selectedId)}/turns/latest`,
      )
        .then((value) => setLatestTurn(value))
        .catch(() => {});
    }, 2_000);
    return () => window.clearInterval(interval);
  }, [selectedId, sending, turnRunning]);

  const startedAt = latestTurn?.started_at ?? null;
  useEffect(() => {
    if (!turnRunning || !startedAt) {
      setElapsed(0);
      return;
    }
    const started = Date.parse(startedAt);
    const update = () =>
      setElapsed(
        Number.isFinite(started)
          ? Math.max(0, Math.round((Date.now() - started) / 1000))
          : 0,
      );
    update();
    const interval = window.setInterval(update, 1_000);
    return () => window.clearInterval(interval);
  }, [turnRunning, startedAt]);

  const refreshThreads = useCallback(async () => {
    if (!activeWorkspaceId) return null;
    try {
      const list = await apiGet<ChatThread[]>(
        `/api/chat/threads?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
      );
      setThreads(list);
      return list;
    } catch {
      // Live refresh is best-effort; the manual Atualizar button remains.
      return null;
    }
  }, [activeWorkspaceId]);

  const reloadSelected = useCallback(async (id: string) => {
    try {
      const value = await apiGet<{ thread: ChatThread; messages: ChatMessage[] }>(
        `/api/chat/threads/${id}`,
      );
      setThread(value.thread);
      setMessages(value.messages);
    } catch {
      // Keep the current view; the thread may have been deleted elsewhere.
    }
  }, []);

  useEffect(() => {
    if (!lastChatEvent || !activeWorkspaceId) return;
    if (lastChatEvent.workspace_id !== activeWorkspaceId) return;
    void (async () => {
      const list = await refreshThreads();
      if (!list) return;
      if (!selectedId) {
        if (list.some((item) => item.id === lastChatEvent.thread_id)) {
          setSelectedId(lastChatEvent.thread_id);
        }
        return;
      }
      if (!list.some((item) => item.id === selectedId)) {
        setSelectedId(null);
        return;
      }
      if (lastChatEvent.thread_id === selectedId) {
        void reloadSelected(selectedId);
      }
    })();
  }, [lastChatEvent, activeWorkspaceId, selectedId, refreshThreads, reloadSelected]);

  const {
    ref: transcriptRef,
    atBottom: transcriptAtBottom,
    onScroll: onTranscriptScroll,
    scrollToBottom: scrollTranscript,
  } = useStickToBottom<HTMLDivElement>(`${messages.length}:${sending}`);

  async function createThread() {
    if (!activeWorkspaceId) return;
    setLoading(true);
    setError(null);
    try {
      const created = await apiPost<ChatThread>("/api/chat/threads", {
        workspace_id: activeWorkspaceId,
        title: "Nova conversa",
        agent,
        model,
        source: "desktop",
      });
      await loadThreads();
      setSelectedId(created.id);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }

  function attachFiles(files: File[]) {
    for (const file of files) {
      if (file.size > 20 * 1024 * 1024) {
        toast({ tone: "error", title: `${file.name} é maior que 20 MB` });
        continue;
      }
      const reader = new FileReader();
      reader.onload = () => {
        setAttachments((current) => [
          ...current,
          { name: file.name, data: String(reader.result ?? "") },
        ]);
      };
      reader.readAsDataURL(file);
    }
  }

  async function send() {
    if (!thread || (!prompt.trim() && attachments.length === 0)) return;
    setSending(true);
    setError(null);
    const content = prompt.trim();
    const files = attachments;
    setPrompt("");
    setAttachments([]);
    try {
      const reply = await apiPost<ChatReply>(
        `/api/chat/threads/${thread.id}/messages`,
        {
          content,
          attachments: files.map((file) => ({
            filename: file.name,
            base64: file.data,
          })),
        },
      );
      setMessages((current) => {
        const known = new Set(current.map((message) => message.id));
        return [
          ...current,
          ...[reply.user, reply.assistant].filter(
            (message) => !known.has(message.id),
          ),
        ];
      });
      setThread(reply.thread);
      if (reply.error) setError(reply.error);
      void apiGet<ChatTurn | null>(
        `/api/chat/threads/${thread.id}/turns/latest`,
      )
        .then((value) => setLatestTurn(value))
        .catch(() => {});
      await loadThreads();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setPrompt(content);
      setAttachments(files);
    } finally {
      setSending(false);
    }
  }

  async function stopExecution() {
    if (!thread) return;
    setStopping(true);
    try {
      await apiPost(`/api/chat/threads/${thread.id}/interrupt`, {});
      toast({
        tone: "warning",
        title: "Interrompendo…",
        description: "O turno atual será encerrado.",
      });
      setLatestTurn((current) =>
        current ? { ...current, status: "CANCELLED" } : current,
      );
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível interromper",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setStopping(false);
    }
  }

  async function renameThread() {
    if (!thread || !titleDraft.trim()) {
      setRenaming(false);
      return;
    }
    try {
      const updated = await apiPatch<ChatThread>(`/api/chat/threads/${thread.id}`, {
        title: titleDraft.trim(),
      });
      setThread(updated);
      setRenaming(false);
      toast({
        tone: "success",
        title: "Conversa renomeada",
        href: `/chat?thread=${encodeURIComponent(thread.id)}`,
      });
      await loadThreads();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível renomear",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function removeThread(id: string) {
    if (!(await confirmAction({ title: "Excluir conversa?", description: "O histórico local desta conversa será removido.", confirmLabel: "Excluir conversa", tone: "destructive" }))) return;
    try {
      await apiDelete(`/api/chat/threads/${id}`);
      if (selectedId === id) setSelectedId(null);
      await loadThreads();
      toast({ tone: "success", title: "Conversa excluída", href: "/chat" });
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(message);
      toast({ tone: "error", title: "Não foi possível excluir", description: message });
    }
  }

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader title="Chat & Criador" />
        <EmptyState
          icon={MessageSquare}
          title="Nenhum workspace ativo"
          description="Selecione um workspace para conversar com agentes."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  return (
    <PageBody>
      <PageHeader
        title="Chat & Criador"
        description={`${workspace?.name ?? "Workspace"} · converse com qualquer agente, com histórico local. Threads do Signal e do Telegram aparecem com o selo correspondente.`}
        actions={
          <Button variant="outline" size="sm" onClick={() => void loadThreads()}>
            <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
            Atualizar
          </Button>
        }
      />

      {error ? (
        <p className="flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="grid gap-3 lg:grid-cols-[320px_1fr]">
        <Card className="flex h-[680px] max-h-[calc(100dvh-190px)] min-h-[460px] flex-col overflow-hidden">
          <CardHeader className="px-4 py-3.5">
            <div>
              <CardTitle>Conversas</CardTitle>
              <p className="mt-0.5 text-[13.5px] text-zinc-500">
                {threads.length} conversa{threads.length === 1 ? "" : "s"}
              </p>
            </div>
            <Button size="xs" onClick={() => void createThread()} disabled={loading}>
              <Plus size={13} />
              Nova
            </Button>
          </CardHeader>
          <div className="border-b border-zinc-800/70 p-2.5">
            <div className="flex h-9 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5 focus-within:border-zinc-600">
              <Search size={13} className="text-zinc-500" />
              <input
                value={threadQuery}
                onChange={(event) => setThreadQuery(event.target.value)}
                placeholder="Buscar conversa ou agente"
                className="h-full flex-1 bg-transparent text-[14px] outline-none placeholder:text-zinc-500"
                aria-label="Buscar conversa ou agente"
              />
            </div>
          </div>
          <CardContent className="min-h-0 flex-1 space-y-1 overflow-y-auto p-2">
            {filteredThreads.length === 0 ? (
              <p className="p-4 text-center text-[13.5px] text-zinc-500">
                {threads.length === 0
                  ? "Nenhuma conversa. Escolha um agente e crie a primeira."
                  : "Nenhuma conversa encontrada."}
              </p>
            ) : (
              filteredThreads.map((item) => (
                <button
                  type="button"
                  key={item.id}
                  onClick={() => setSelectedId(item.id)}
                  className={cn(
                    "w-full rounded-lg px-3 py-2.5 text-left transition-colors",
                    selectedId === item.id
                      ? "bg-zinc-800 text-zinc-100"
                      : "text-zinc-400 hover:bg-zinc-900",
                  )}
                >
                  <div className="flex items-center gap-1.5">
                    <p className="min-w-0 flex-1 truncate text-[13.5px]">{item.title}</p>
                    {item.source !== "desktop" ? (
                      <Badge variant="purple">{item.source}</Badge>
                    ) : null}
                  </div>
                  <p className="mt-0.5 truncate font-mono text-[11.5px] text-zinc-500">
                    @{item.agent} · {relativeTime(item.last_message_at ?? item.updated_at)}
                  </p>
                </button>
              ))
            )}
          </CardContent>
          <div className="space-y-2 border-t border-zinc-800/70 p-3">
            <div className="block">
              <span className={labelClass}>Agente para novas conversas</span>
              <AgentCombobox
                value={agent}
                onChange={(value) => setAgent(value)}
                workspaceId={activeWorkspaceId}
                placeholder="build"
                allowEmpty={false}
              />
            </div>
            <div className="block">
              <span className={labelClass}>Modelo (opcional)</span>
              <ModelCombobox
                value={model}
                onChange={setModel}
                routerLabel="Automático"
                routerHint="Sem modelo fixo, a conversa usa o modelo padrão do agente no OpenCode; o roteador DSH não interfere."
              />
            </div>
            <Button size="sm" className="w-full" onClick={() => void createThread()} disabled={loading}>
              {loading ? <Loader2 size={13} className="animate-spin" /> : <Plus size={13} />}
              Criar conversa com @{agent}
            </Button>
          </div>
        </Card>

        <Card className="flex h-[680px] max-h-[calc(100dvh-190px)] min-h-[460px] flex-col overflow-hidden">
          <CardHeader className="px-4 py-3">
            <div className="min-w-0 flex-1">
              {renaming && thread ? (
                <div className="flex items-center gap-1">
                  <input
                    autoFocus
                    value={titleDraft}
                    onChange={(event) => setTitleDraft(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void renameThread();
                      if (event.key === "Escape") setRenaming(false);
                    }}
                    className="h-8 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2.5 text-[13.5px] text-zinc-100 outline-none"
                  />
                  <Button variant="ghost" size="icon" className="h-8 w-8" aria-label="Confirmar novo nome" onClick={() => void renameThread()}>
                    <Check size={14} />
                  </Button>
                  <Button variant="ghost" size="icon" className="h-8 w-8" aria-label="Cancelar renomeação" onClick={() => setRenaming(false)}>
                    <X size={14} />
                  </Button>
                </div>
              ) : (
                <CardTitle className="truncate text-[13.5px]">
                  {thread?.title ?? "Selecione uma conversa"}
                </CardTitle>
              )}
              {thread ? (
                <p className="mt-0.5 flex flex-wrap items-center gap-1 font-mono text-[13px] text-zinc-500">
                  <EntityLink
                    to={links.agent(thread.agent, "workspace", activeWorkspaceId)}
                    chip
                  >
                    @{thread.agent}
                  </EntityLink>
                  {thread.model ? <span>· {thread.model}</span> : null}
                  {thread.opencode_session_id
                    ? ` · sessão ${thread.opencode_session_id.slice(0, 12)}`
                    : " · sessão será criada no primeiro envio"}
                </p>
              ) : null}
            </div>
            <div className="flex items-center gap-2">
              {thread ? (
                <Button
                  variant="ghost"
                  size="sm"
                  title="Renomear conversa"
                  onClick={() => {
                    setTitleDraft(thread.title);
                    setRenaming(true);
                  }}
                >
                  <Pencil size={13} />
                </Button>
              ) : null}
              {thread ? (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => void removeThread(thread.id)}
                >
                  <Trash2 size={13} />
                </Button>
              ) : null}
              <Badge variant={thinking ? "yellow" : "blue"}>
                {thinking ? "pensando…" : "pronto"}
              </Badge>
              {latestTurn && !thinking ? (
                <span className="text-[11.5px] text-zinc-500">
                  última execução: {latestTurn.status}
                  {latestTurn.run_id ? (
                    <Link
                      to={links.run(latestTurn.run_id)}
                      className="ml-1 underline decoration-zinc-600 underline-offset-2 hover:text-zinc-300"
                    >
                      ver
                    </Link>
                  ) : null}
                </span>
              ) : null}
            </div>
          </CardHeader>
          <CardContent className="flex min-h-0 flex-1 flex-col gap-3 p-0">
            <div className="relative min-h-0 flex-1 overflow-hidden">
            <div
              ref={transcriptRef}
              data-testid="chat-transcript"
              onScroll={onTranscriptScroll}
              className="h-full min-h-0 space-y-3 overflow-y-auto overscroll-contain px-4 py-3"
            >
              {!thread ? (
                <EmptyState
                  icon={Bot}
                  title="Escolha ou crie uma conversa"
                  description="Cada conversa usa um agente e mantém o histórico no SQLite. O conteúdo é executado pelo OpenCode."
                />
              ) : messages.length === 0 ? (
                <EmptyState
                  icon={Sparkles}
                  title="Comece a conversa"
                  description={`Envie a primeira mensagem para @${thread.agent}.`}
                />
              ) : (
                messages.map((message) => (
                  <div
                    key={message.id}
                    className={cn(
                      "max-w-[85%] rounded-xl px-3.5 py-2.5 text-[13.5px] leading-relaxed",
                      message.role === "user"
                        ? "ml-auto bg-blue-500/15 text-blue-100"
                        : "border border-zinc-800/70 bg-zinc-900/50 text-zinc-300",
                    )}
                  >
                    <p className="whitespace-pre-wrap">{message.content}</p>
                    <p className="mt-1 text-right font-mono text-[11px] text-zinc-500">
                      {message.role === "user" ? "você" : "agente"} ·{" "}
                      {relativeTime(message.created_at)}
                    </p>
                  </div>
                ))
              )}
              {thinking ? (
                <div className="flex flex-wrap items-center gap-2 rounded-lg border border-sky-500/20 bg-sky-500/[0.05] px-3 py-2 text-[11.5px] text-sky-200/90">
                  <Loader2 size={12} className="animate-spin" />
                  O agente está pensando
                  {elapsed > 0 ? ` · ${elapsed}s` : ""}…
                  {latestTurn?.run_id ? (
                    <Link
                      to={links.run(latestTurn.run_id)}
                      className="underline decoration-sky-400/40 underline-offset-2 hover:text-sky-100"
                    >
                      ver execução
                    </Link>
                  ) : null}
                  <Button
                    size="xs"
                    variant="outline"
                    className="ml-auto border-red-500/30 text-red-200"
                    onClick={() => void stopExecution()}
                    disabled={stopping}
                  >
                    <Square size={10} />
                    {stopping ? "parando…" : "Parar"}
                  </Button>
                </div>
              ) : null}
            </div>
            {!transcriptAtBottom ? (
              <Button
                variant="outline"
                size="sm"
                className="absolute bottom-3 right-4 h-8 gap-1.5 rounded-full bg-zinc-950/95 shadow-lg"
                onClick={() => scrollTranscript()}
                title="Ir para a mensagem mais recente"
              >
                <ArrowDown size={13} />
                ir para o fim
              </Button>
            ) : null}
            </div>
            <div
              data-testid="chat-composer"
              className="shrink-0 border-t border-zinc-800/70 p-3"
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => {
                event.preventDefault();
                attachFiles(Array.from(event.dataTransfer.files));
              }}
              onPaste={(event) => {
                const files = Array.from(event.clipboardData.files);
                if (files.length > 0) {
                  event.preventDefault();
                  attachFiles(files);
                }
              }}
            >
              {attachments.length > 0 ? (
                <div className="mb-2 flex flex-wrap gap-1.5">
                  {attachments.map((file, index) => (
                    <span
                      key={`${file.name}-${index}`}
                      className="inline-flex items-center gap-1 rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1 text-[11.5px] text-zinc-400"
                    >
                      <Paperclip size={10} />
                      <span className="max-w-40 truncate">{file.name}</span>
                      <button
                        type="button"
                        onClick={() =>
                          setAttachments((current) =>
                            current.filter((_, itemIndex) => itemIndex !== index),
                          )
                        }
                        className="text-zinc-500 hover:text-zinc-200"
                      >
                        <X size={10} />
                      </button>
                    </span>
                  ))}
                </div>
              ) : null}
              <div className="flex w-full items-stretch gap-2">
                <SlashTextarea
                  containerClassName="min-w-0 flex-1"
                  value={prompt}
                  onChange={setPrompt}
                  workspaceId={activeWorkspaceId}
                  rows={2}
                  submitOnEnter
                  onSubmit={() => void send()}
                  disabled={!thread || sending}
                  placeholder={
                    thread
                      ? `Mensagem para @${thread.agent}… (Enter envia, Shift+Enter quebra linha, cole/arraste arquivos)`
                      : "Crie ou selecione uma conversa primeiro"
                  }
                  className="min-h-[52px] flex-1"
                />
                <Button
                  size="sm"
                  className="h-auto min-h-[52px] w-24 shrink-0"
                  onClick={() => void send()}
                  disabled={!thread || sending || (!prompt.trim() && attachments.length === 0)}
                >
                  {sending ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <Send size={14} />
                  )}
                  Enviar
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Wand2 size={15} className="text-zinc-500" />
              Meta-Builder
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Crie agentes, skills e comandos descrevendo o pedido em linguagem natural.
            </p>
          </div>
          <Badge variant="purple">disco</Badge>
        </CardHeader>
        <CardContent>
          <MetaBuilder workspaceId={activeWorkspaceId} />
        </CardContent>
      </Card>
    </PageBody>
  );
}

function MetaBuilder({ workspaceId }: { workspaceId: string }) {
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    kind: string;
    id: string;
    path: string;
    message: string;
  } | null>(null);

  async function build() {
    if (!prompt.trim()) {
      setError("Descreva o agente, skill ou comando.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const value = await apiPost<typeof result>(
        `/api/meta/build?workspace_id=${encodeURIComponent(workspaceId)}&scope=workspace`,
        { prompt: prompt.trim() },
      );
      setResult(value);
      setPrompt("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-3">
      <div className="flex w-full items-stretch gap-2">
        <SlashTextarea
          containerClassName="min-w-0 flex-1"
          value={prompt}
          onChange={setPrompt}
          workspaceId={workspaceId}
          rows={2}
          placeholder="Crie um subagente chamado revisor-fiscal focado em contabilidade…"
          className="min-h-[52px] flex-1"
        />
        <Button
          size="sm"
          className="h-auto min-h-[52px] w-28 shrink-0"
          onClick={() => void build()}
          disabled={busy}
        >
          {busy ? <Loader2 size={14} className="animate-spin" /> : <Wand2 size={14} />}
          Criar arquivo
        </Button>
      </div>
      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}
      {result ? (
        <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/[0.06] p-3 text-[13px] text-emerald-200">
          <p className="font-medium">{result.message}</p>
          <p className="mt-1 font-mono text-[11.5px] text-emerald-300/70">
            {result.kind} · {result.id} · {result.path}
          </p>
        </div>
      ) : null}
    </div>
  );
}


