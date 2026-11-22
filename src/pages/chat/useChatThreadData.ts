import { useCallback, useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { apiGet } from "@/lib/api";
import type { ChatMessage, ChatThread, ChatTurn } from "@/lib/types";
import type { AppStore } from "@/store/app-store-types";

export function useChatThreadData({
  activeWorkspaceId,
  lastChatEvent,
  setAgent,
  setModel,
}: {
  activeWorkspaceId: string | null;
  lastChatEvent: AppStore["lastChatEvent"];
  setAgent: (value: string) => void;
  setModel: (value: string | null) => void;
}) {
  const [threads, setThreads] = useState<ChatThread[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [thread, setThread] = useState<ChatThread | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [latestTurn, setLatestTurn] = useState<ChatTurn | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [params, setParams] = useSearchParams();

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
  }, [selectedId, setAgent, setModel]);

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

  const turnRunning = latestTurn?.status === "RUNNING";

  return {
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
  };
}
