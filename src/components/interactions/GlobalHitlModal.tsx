import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { HitlCard } from "@/components/dashboard/attention/HitlCard";
import {
  EMPTY_ATTENTION,
  type Attention,
  type HitlRequest,
} from "@/components/dashboard/attention/types";
import { InteractionCard } from "@/components/interactions/InteractionCard";
import { apiGet, apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import {
  markNotificationOnce,
  notificationEnabled,
  sendNativeNotification,
} from "@/lib/notification-policy";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";

/// A single place to answer HITL from any page. The Dashboard keeps the full
/// attention list; this modal surfaces the newest pending decision so the user
/// does not have to navigate while a run is blocked.
export function GlobalHitlModal() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastRecipeEvent = useAppStore((state) => state.lastRecipeEvent);
  const lastPermissionEvent = useAppStore((state) => state.lastPermissionEvent);
  const [data, setData] = useState<Attention>(EMPTY_ATTENTION);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [feedbackToken, setFeedbackToken] = useState<string | null>(null);
  const [feedbackText, setFeedbackText] = useState("");
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());
  const known = useRef<Set<string>>(new Set());

  const load = useCallback(async () => {
    if (!workspaceId) return;
    try {
      const attention = await apiGet<Attention>(
        `/api/attention?workspace_id=${encodeURIComponent(workspaceId)}`,
      );
      const merged = { ...EMPTY_ATTENTION, ...attention };
      setData(merged);

      const fresh = merged.task_interactions.filter(
        (interaction) => !known.current.has(interaction.id),
      );
      for (const interaction of merged.task_interactions) {
        known.current.add(interaction.id);
      }
      if (fresh.length > 0 && notificationEnabled("interactions")) {
        const first = fresh[0];
        if (markNotificationOnce(`interaction:${first.id}`)) {
          void sendNativeNotification(
            "O agente precisa da sua resposta",
            first.question.slice(0, 140),
          );
        }
      }

      const pendingInteraction = merged.task_interactions.find(
        (interaction) => !dismissed.has(interaction.id),
      );
      const pendingHitl = merged.hitl.find(
        (request) => !dismissed.has(request.token),
      );
      if (pendingInteraction || pendingHitl) {
        setOpen(true);
      }
    } catch {
      // Offline is fine: the modal keeps its current state.
    }
  }, [workspaceId, dismissed]);

  useEffect(() => {
    void load();
  }, [load, lastTaskEvent, lastRecipeEvent, lastPermissionEvent]);

  useEffect(() => {
    const timer = window.setInterval(() => void load(), 60_000);
    return () => window.clearInterval(timer);
  }, [load]);

  useEffect(() => {
    if (!workspaceId) {
      setOpen(false);
      setData(EMPTY_ATTENTION);
      known.current.clear();
      setDismissed(new Set());
    }
  }, [workspaceId]);

  const interaction =
    data.task_interactions.find((item) => !dismissed.has(item.id)) ?? null;
  const hitl =
    data.hitl.find((request) => !dismissed.has(request.token)) ?? null;

  function dismissOne(id: string) {
    setDismissed((current) => new Set(current).add(id));
  }

  function dismiss() {
    if (interaction) dismissOne(interaction.id);
    else if (hitl) dismissOne(hitl.token);
    setOpen(false);
  }

  async function resolveHitl(
    request: HitlRequest,
    resolution: string,
    options: { text?: string; optionIndex?: number } = {},
  ) {
    setBusy(true);
    try {
      await apiPost(`/api/recipes/hitl/${request.token}/resolve`, {
        resolution,
        text: options.text,
        option_index: options.optionIndex,
      });
      if (feedbackToken === request.token) {
        setFeedbackToken(null);
        setFeedbackText("");
      }
      toast({
        tone: "success",
        title:
          resolution === "APPROVED"
            ? "Checkpoint aprovado"
            : resolution === "REJECTED"
              ? "Checkpoint negado"
              : resolution === "CHOICE"
                ? "Opção registrada"
                : "Payload enviado ao agente",
      });
      dismissOne(request.token);
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível responder",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(false);
    }
  }

  const current = interaction ? "interaction" : hitl ? "hitl" : null;

  return (
    <Modal
      open={open && current !== null}
      onClose={dismiss}
      title="Decisão necessária"
      description="Uma execução está pausada esperando você. Responda aqui ou veja todos os alertas no Dashboard."
      size="lg"
      initialFocus="dialog"
    >
      <div className="scroll-area max-h-[70vh] space-y-3 overflow-y-auto pr-0.5">
        {interaction ? (
          <InteractionCard
            key={interaction.id}
            interaction={interaction}
            onResolved={() => {
              dismissOne(interaction.id);
              void load();
            }}
          />
        ) : null}
        {!interaction && hitl ? (
          <HitlCard
            key={hitl.token}
            request={hitl}
            busy={busy}
            feedbackOpen={feedbackToken === hitl.token}
            feedbackText={feedbackText}
            onFeedbackText={setFeedbackText}
            onToggleFeedback={() => {
              setFeedbackToken((value) =>
                value === hitl.token ? null : hitl.token,
              );
              setFeedbackText("");
            }}
            onApprove={() => void resolveHitl(hitl, "APPROVED")}
            onReject={() => void resolveHitl(hitl, "REJECTED")}
            onChoose={(index, value) =>
              void resolveHitl(hitl, "CHOICE", {
                text: value,
                optionIndex: index,
              })
            }
            onFeedback={() => {
              const value = feedbackText.trim();
              if (value) void resolveHitl(hitl, "FEEDBACK", { text: value });
            }}
          />
        ) : null}
      </div>
      <div className="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-zinc-800/70 pt-3">
        <Link
          to="/"
          onClick={dismiss}
          className="inline-flex items-center gap-1.5 text-[12px] text-zinc-400 transition-colors hover:text-zinc-200"
        >
          <AlertTriangle size={12} />
          Ver todos os alertas
        </Link>
        <div className="flex items-center gap-1.5">
          {interaction ? (
            <Link
              to={links.task(interaction.task_id)}
              onClick={dismiss}
              className="text-[12px] text-zinc-500 transition-colors hover:text-zinc-300"
            >
              abrir tarefa
            </Link>
          ) : null}
          <Button variant="ghost" size="sm" onClick={dismiss}>
            Depois
          </Button>
        </div>
      </div>
    </Modal>
  );
}
