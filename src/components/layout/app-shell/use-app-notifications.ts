import { useEffect } from "react";
import { useLocation } from "react-router-dom";
import { apiGet } from "@/lib/api";
import { links } from "@/lib/links";
import {
  artifactNotificationsEnabled,
  isTerminalTaskStatus,
  markNotificationOnce,
  notificationEnabled,
  sendNativeNotification,
} from "@/lib/notification-policy";
import { runNotification } from "@/lib/run-notification";
import type { Schedule, Task, TaskRun } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { toast, useToastStore } from "@/store/toast-store";

/**
 * Watches live store events and turns them into toasts/native notifications.
 * Extracted from AppShell so the shell itself stays a layout component.
 */
export function useAppNotifications() {
  const budgetAlert = useAppStore((state) => state.budgetAlert);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastEngineUpdate = useAppStore((state) => state.lastEngineUpdate);
  const lastArtifact = useAppStore((state) => state.lastArtifact);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastGoogleEvent = useAppStore((state) => state.lastGoogleEvent);
  const lastScheduleEvent = useAppStore((state) => state.lastScheduleEvent);
  const location = useLocation();

  useEffect(() => {
    if (!lastRunEvent) return;
    // Tasks go through Plan/Build/Audit runs. Only the terminal task status
    // notifies; runs without a task (schedules) notify once per run.
    if (lastRunEvent.task_id) return;
    const status = lastRunEvent.status;
    const enabled =
      status === "SUCCESS"
        ? notificationEnabled("run_finished")
        : notificationEnabled("run_failed");
    if (!enabled) return;
    const key = `run:${lastRunEvent.run_id}:${status}`;
    if (!markNotificationOnce(key)) return;
    void (async () => {
      let run: TaskRun | null = null;
      try {
        run = await apiGet<TaskRun>(
          `/api/runs/${encodeURIComponent(lastRunEvent.run_id)}`,
        );
      } catch {
        // Fall back to the generic message.
      }
      const notification = runNotification(
        status,
        run?.task_title ?? "Execução",
        run,
      );
      toast({
        tone: status === "SUCCESS" ? "success" : status === "FAILED" ? "error" : "warning",
        title: notification.title,
        description: notification.body,
        href: links.run(lastRunEvent.run_id),
        key,
      });
      void sendNativeNotification(notification.title, notification.body);
    })();
  }, [lastRunEvent]);

  useEffect(() => {
    const event = lastTaskEvent;
    if (!event || !isTerminalTaskStatus(event.stage)) return;
    const enabled =
      event.stage === "DONE"
        ? notificationEnabled("run_finished")
        : notificationEnabled("run_failed");
    if (!enabled) return;
    const key = `task:${event.task_id}:${event.stage}`;
    if (!markNotificationOnce(key)) return;
    void (async () => {
      let task: Task | null = null;
      let run: TaskRun | null = null;
      try {
        task = await apiGet<Task>(`/api/tasks/${encodeURIComponent(event.task_id)}`);
        const runs = await apiGet<TaskRun[]>(
          `/api/runs?workspace_id=${encodeURIComponent(event.workspace_id)}&task_id=${encodeURIComponent(event.task_id)}&limit=1`,
        );
        run = runs[0] ?? null;
      } catch {
        // The notification still explains the terminal state.
      }
      const notification = runNotification(
        event.stage,
        task?.title ?? "Tarefa",
        run
          ? { ...run, task_title: task?.title ?? run.task_title }
          : null,
      );
      toast({
        tone: event.stage === "DONE" ? "success" : event.stage === "BLOCKED" ? "error" : "warning",
        title: notification.title,
        description: notification.body,
        href: links.task(event.task_id),
        key,
      });
      void sendNativeNotification(notification.title, notification.body);
    })();
  }, [lastTaskEvent]);

  useEffect(() => {
    if (!lastEngineUpdate) return;
    if (!["done", "failed"].includes(lastEngineUpdate.phase)) return;
    const key = `engine:${lastEngineUpdate.phase}:${lastEngineUpdate.message ?? ""}`;
    if (!markNotificationOnce(key)) return;
    const title =
      lastEngineUpdate.ok === false || lastEngineUpdate.phase === "failed"
        ? "Atualização falhou"
        : "Atualização concluída";
    toast({
      tone: title.includes("falhou") ? "error" : "success",
      title,
      description: lastEngineUpdate.message ?? undefined,
      href: links.settings("engine"),
      key,
    });
    void sendNativeNotification(title, lastEngineUpdate.message ?? "");
  }, [lastEngineUpdate]);

  useEffect(() => {
    if (!lastArtifact || location.pathname === "/artifacts") return;
    if (!artifactNotificationsEnabled()) return;
    const key = `artifacts:${lastArtifact.run_id || lastArtifact.id}`;
    if (!markNotificationOnce(key)) return;
    toast({
      tone: "info",
      title: "Artefato criado",
      description: lastArtifact.file_path,
      href: links.artifact(lastArtifact.id),
      key,
      duration: 3200,
    });
  }, [lastArtifact, location.pathname]);

  useEffect(() => {
    if (!lastGoogleEvent) return;
    const key = `google:${lastGoogleEvent.workspace_id}:${lastGoogleEvent.connected}`;
    if (!markNotificationOnce(key)) return;
    const title = lastGoogleEvent.connected
      ? "Google conectado"
      : "Google desconectado";
    toast({
      tone: lastGoogleEvent.connected ? "success" : "warning",
      title,
      description: lastGoogleEvent.email ?? undefined,
      href: links.integration("google"),
      key,
    });
    void sendNativeNotification(title, lastGoogleEvent.email ?? "");
  }, [lastGoogleEvent]);

  useEffect(() => {
    if (!lastScheduleEvent) return;
    if (!notificationEnabled("schedule_skips")) return;
    let cancelled = false;
    void apiGet<Schedule[]>(
      `/api/schedules?workspace_id=${encodeURIComponent(lastScheduleEvent.workspace_id)}`,
    )
      .then((list) => {
        if (cancelled) return;
        const schedule = list.find(
          (item) => item.id === lastScheduleEvent.schedule_id,
        );
        const status = schedule?.last_run_status ?? "";
        const stamp = schedule?.last_run_at ?? "";
        if (
          !stamp ||
          !["FAILED", "REJECTED", "TIMED_OUT", "SKIPPED_OFFLINE", "INVALID_CRON"].includes(
            status,
          )
        ) {
          return;
        }
        const key = `schedule:${lastScheduleEvent.schedule_id}:${stamp}:${status}`;
        if (!markNotificationOnce(key)) return;
        const title =
          status === "SKIPPED_OFFLINE"
            ? "Rotina pulada: engine offline"
            : status === "REJECTED"
              ? "Rotina rejeitada no checkpoint"
              : status === "TIMED_OUT"
                ? "Rotina expirou no checkpoint"
                : "Rotina falhou";
        toast({
          tone: status === "SKIPPED_OFFLINE" ? "warning" : "error",
          title,
          description: schedule?.name,
          href: links.schedule(lastScheduleEvent.schedule_id),
          key,
        });
        void sendNativeNotification(title, schedule?.name ?? "");
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [lastScheduleEvent]);

  useEffect(() => {
    if (!budgetAlert) return;
    const key = `budget:${budgetAlert.level}:${budgetAlert.message}`;
    if (!markNotificationOnce(key)) return;
    const title =
      budgetAlert.level === "hard"
        ? "Orçamento bloqueado"
        : "Orçamento próximo do limite";
    toast({
      tone: budgetAlert.level === "hard" ? "error" : "warning",
      title,
      description: budgetAlert.message,
      href: "/router",
      key,
      duration: budgetAlert.level === "hard" ? 0 : 6000,
    });
    void sendNativeNotification(title, budgetAlert.message);
  }, [budgetAlert]);

  useEffect(() => {
    const clear = () => useToastStore.getState().clear();
    const onVisibility = () => {
      if (document.hidden) clear();
    };
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("beforeunload", clear);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("beforeunload", clear);
    };
  }, []);
}
