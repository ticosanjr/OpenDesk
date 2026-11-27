import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { MessageSquare, RefreshCw, Wand2 } from "lucide-react";
import { PageBody, PageBodyFill, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { SectionTabs } from "@/components/ui/section-tabs";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
import type { ChatReply, ChatThread, ChatTurn } from "@/lib/types";
import { toast } from "@/store/toast-store";
import { confirmAction } from "@/store/confirm-store";
import { useStickToBottom } from "@/lib/use-stick-to-bottom";
import { ChatComposer, type ChatAttachment } from "./chat/ChatComposer";
import { EmailReplyModal } from "./chat/EmailReplyModal";
import { MetaBuilder } from "./chat/MetaBuilder";
import { ThreadHeader } from "./chat/ThreadHeader";
import { ThreadListCard } from "./chat/ThreadListCard";
import { Transcript } from "./chat/Transcript";
import { useChatThreadData } from "./chat/useChatThreadData";
import { useVoiceOutput } from "@/lib/use-voice-output";

export function ChatPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const lastChatEvent = useAppStore((state) => state.lastChatEvent);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );

  const [params, setParams] = useSearchParams();
  const tab = params.get("tab") === "builder" ? "builder" : "chat";
  function setTab(next: string) {
    const search = new URLSearchParams(params);
    if (next === "builder") search.set("tab", "builder");
    else search.delete("tab");
    setParams(search, { replace: true });
  }
  const [agent, setAgent] = useState("build");
  const [model, setModel] = useState<string | null>(null);
  const [attachments, setAttachments] = useState<ChatAttachment[]>([]);
  const [prompt, setPrompt] = useState("");
  const [sending, setSending] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [threadQuery, setThreadQuery] = useState("");
  const [renaming, setRenaming] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [emailOpen, setEmailOpen] = useState(false);
  const [emailSubject, setEmailSubject] = useState("");
  const { play: playVoice } = useVoiceOutput();
  const {
    threads,
    selectedId,
    setSelectedId,
    thread,
    setThread,
    messages,
    setMessages,
    loading,
    setLoading,
    latestTurn,
    setLatestTurn,
    error,
    setError,
    turnRunning,
    loadThreads,
  } = useChatThreadData({
    activeWorkspaceId,
    lastChatEvent,
    setAgent,
    setModel,
  });

  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const thinking = sending || turnRunning;

  useEffect(() => {
    if (!selectedId || (!sending && !turnRunning)) return;
    const interval = window.setInterval(() => {
      void apiGet<ChatTurn | null>(
        `/api/chat/threads/${encodeURIComponent(selectedId)}/turns/latest`,
      )
        .then((value) => setLatestTurn(value))
        .catch(() => {});
    }, 15_000);
    return () => window.clearInterval(interval);
  }, [selectedId, sending, turnRunning, setLatestTurn]);

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

  function startRename() {
    if (!thread) return;
    setTitleDraft(thread.title);
    setRenaming(true);
  }

  function replyEmail() {
    if (!thread) return;
    setEmailSubject(`Re: ${thread.title.replace(/^E-mail · /, "")}`);
    setEmailOpen(true);
  }

  function removeSelectedThread() {
    if (thread) void removeThread(thread.id);
  }

  function removeAttachment(index: number) {
    setAttachments((current) =>
      current.filter((_, itemIndex) => itemIndex !== index),
    );
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
    <PageBodyFill>
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

      <SectionTabs
        value={tab}
        onChange={setTab}
        items={[
          { id: "chat", label: "Chat", icon: MessageSquare },
          { id: "builder", label: "Criador", icon: Wand2 },
        ]}
      />

      {error ? (
        <p className="flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
          {error}
        </p>
      ) : null}

      {tab === "chat" ? (
      <div className="grid min-h-0 flex-1 gap-3 lg:grid-cols-[320px_1fr]">
        <ThreadListCard
          threads={threads}
          threadQuery={threadQuery}
          onThreadQueryChange={setThreadQuery}
          selectedId={selectedId}
          onSelect={setSelectedId}
          agent={agent}
          onAgentChange={setAgent}
          model={model}
          onModelChange={setModel}
          workspaceId={activeWorkspaceId}
          loading={loading}
          onCreate={() => void createThread()}
        />

        <Card className="flex h-full min-h-0 flex-col overflow-hidden">
          <ThreadHeader
            thread={thread}
            workspaceId={activeWorkspaceId}
            renaming={renaming}
            titleDraft={titleDraft}
            onTitleDraftChange={setTitleDraft}
            onRename={() => void renameThread()}
            onCancelRename={() => setRenaming(false)}
            onStartRename={startRename}
            onReplyEmail={replyEmail}
            onRemove={removeSelectedThread}
            thinking={thinking}
            latestTurn={latestTurn}
          />
          <CardContent className="flex min-h-0 flex-1 flex-col gap-3 p-0">
            <Transcript
              thread={thread}
              messages={messages}
              thinking={thinking}
              turnRunning={turnRunning}
              latestTurn={latestTurn}
              stopping={stopping}
              transcriptRef={transcriptRef}
              onTranscriptScroll={onTranscriptScroll}
              transcriptAtBottom={transcriptAtBottom}
              onScrollToBottom={() => scrollTranscript()}
              onSpeak={(text) => void playVoice(text, activeWorkspaceId)}
              onStop={() => void stopExecution()}
            />
            <ChatComposer
              thread={thread}
              workspaceId={activeWorkspaceId}
              prompt={prompt}
              onPromptChange={setPrompt}
              attachments={attachments}
              onFiles={attachFiles}
              onRemoveAttachment={removeAttachment}
              sending={sending}
              onSend={() => void send()}
            />
          </CardContent>
        </Card>
      </div>
      ) : null}

      <EmailReplyModal
        open={emailOpen}
        onClose={() => setEmailOpen(false)}
        thread={thread}
        subject={emailSubject}
        onSubjectChange={setEmailSubject}
      />

      {tab === "builder" ? (
      <Card className="flex min-h-0 flex-1 flex-col">
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
        <CardContent className="flex min-h-0 flex-1 flex-col">
          <MetaBuilder workspaceId={activeWorkspaceId} />
        </CardContent>
      </Card>
      ) : null}
    </PageBodyFill>
  );
}
