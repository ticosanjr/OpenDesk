import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useNavigate } from "react-router-dom";
import {
  Activity,
  Archive,
  Brain,
  CalendarClock,
  CalendarDays,
  FolderTree,
  Gauge,
  KanbanSquare,
  MessageSquare,
  Network,
  Plug,
  Plus,
  RefreshCw,
  Route,
  RotateCw,
  Settings,
  Search,
  Wand2,
  Workflow,
} from "lucide-react";
import { useUiStore } from "@/store/ui-store";
import { useAppStore } from "@/store/app-store";
import { apiGet } from "@/lib/api";
import type { AgentFile, Artifact, Schedule, Task, TaskRun } from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { NAV_ITEMS } from "@/components/layout/Sidebar";
import { cn } from "@/lib/utils";

const icons = {
  "/": Gauge,
  "/search": Search,
  "/kanban": KanbanSquare,
  "/calendar": CalendarDays,
  "/runs": Activity,
  "/chat": MessageSquare,
  "/artifacts": Archive,
  "/files": FolderTree,
  "/routines": CalendarClock,
  "/recipes": Workflow,
  "/studio": Wand2,
  "/governance": Network,
  "/observability": Activity,
  "/learning": Brain,
  "/router": Route,
  "/integrations": Plug,
  "/settings": Settings,
} as const;

export function CommandPalette() {
  const open = useUiStore((state) => state.commandOpen);
  const setOpen = useUiStore((state) => state.setCommandOpen);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const refresh = useAppStore((state) => state.refresh);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);
  const navigate = useNavigate();
  const [tasks, setTasks] = useState<Task[]>([]);
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [schedules, setSchedules] = useState<Schedule[]>([]);
  const [agents, setAgents] = useState<AgentFile[]>([]);
  const [runs, setRuns] = useState<TaskRun[]>([]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, setOpen]);

  useEffect(() => {
    if (!open || !activeWorkspaceId) return;
    let cancelled = false;
    const query = `workspace_id=${encodeURIComponent(activeWorkspaceId)}`;
    void Promise.all([
      apiGet<Task[]>(`/api/tasks?${query}`).catch(() => []),
      apiGet<Artifact[]>(`/api/artifacts?${query}&limit=40`).catch(() => []),
      apiGet<Schedule[]>(`/api/schedules?${query}`).catch(() => []),
      apiGet<AgentFile[]>(
        `/api/studio/agents?scope=workspace&workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
      ).catch(() => []),
      apiGet<TaskRun[]>(`/api/runs?${query}`).catch(() => []),
    ]).then(([taskList, artifactList, scheduleList, agentList, runList]) => {
      if (cancelled) return;
      setTasks(taskList);
      setArtifacts(artifactList);
      setSchedules(scheduleList);
      setAgents(agentList);
      setRuns(runList);
    });
    return () => {
      cancelled = true;
    };
  }, [activeWorkspaceId, open]);

  if (!open) return null;

  function run(action: () => void) {
    setOpen(false);
    action();
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <button
        type="button"
        aria-label="Fechar command bar"
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={() => setOpen(false)}
      />
      <Command
        label="Command Menu"
        className="relative w-full max-w-4xl overflow-hidden rounded-2xl border border-zinc-800 bg-zinc-950/95 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] backdrop-blur-xl"
      >
        <div className="flex items-center gap-2 border-b border-zinc-800/70 px-3">
          <Search size={15} className="text-zinc-500" />
          <Command.Input
            autoFocus
            placeholder="Ir para, criar workspace ou reiniciar o engine…"
            className="h-11 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
          />
          <span className="rounded border border-zinc-800 bg-zinc-900 px-1.5 py-0.5 font-mono text-[9px] text-zinc-500">
            ESC
          </span>
        </div>

        <Command.List className="max-h-[min(60dvh,28rem)] overflow-y-auto p-2">
          <Command.Empty className="px-3 py-8 text-center text-xs text-zinc-500">
            Nenhum resultado.
          </Command.Empty>

          <Command.Group
            heading="Navegação"
            className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[9px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-500"
          >
            {NAV_ITEMS.map((item) => {
              const Icon = icons[item.to as keyof typeof icons] ?? Gauge;
              return (
                <Command.Item
                  key={item.to}
                  value={item.label}
                  onSelect={() => run(() => navigate(item.to))}
                  className={cn(
                    "flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none",
                    "data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100",
                  )}
                >
                  <Icon size={15} className="text-zinc-500" />
                  {item.label}
                </Command.Item>
              );
            })}
          </Command.Group>

          {tasks.length > 0 ? (
            <Command.Group
              heading="Tarefas"
              className="mt-2 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[9px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-500"
            >
              {tasks.slice(0, 12).map((task) => (
                <Command.Item
                  key={task.id}
                  value={`tarefa ${task.title} ${task.assigned_agent ?? ""}`}
                  onSelect={() =>
                    run(() => navigate(`/kanban?task=${encodeURIComponent(task.id)}`))
                  }
                  className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[12px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
                >
                  <KanbanSquare size={14} className="shrink-0 text-zinc-500" />
                  <span className="min-w-0 flex-1 truncate">{task.title}</span>
                  <span className="shrink-0 font-mono text-[9px] text-zinc-500">
                    {task.status}
                  </span>
                </Command.Item>
              ))}
            </Command.Group>
          ) : null}

          {runs.length > 0 ? (
            <Command.Group
              heading="Execuções"
              className="mt-2 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[9px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-500"
            >
              {runs.slice(0, 10).map((item) => (
                <Command.Item
                  key={item.id}
                  value={`run execução ${item.task_title ?? ""} ${item.agent_name} ${item.model_id} ${item.status}`}
                  onSelect={() =>
                    run(() => navigate(`/runs/${encodeURIComponent(item.id)}`))
                  }
                  className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[12px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
                >
                  <Activity size={14} className="shrink-0 text-zinc-500" />
                  <span className="min-w-0 flex-1 truncate">
                    {item.task_title ?? item.agent_name}
                  </span>
                  <span className="shrink-0 font-mono text-[9px] text-zinc-500">
                    {item.status}
                  </span>
                </Command.Item>
              ))}
            </Command.Group>
          ) : null}

          {schedules.length > 0 ? (
            <Command.Group
              heading="Rotinas"
              className="mt-2 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[9px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-500"
            >
              {schedules.slice(0, 10).map((schedule) => (
                <Command.Item
                  key={schedule.id}
                  value={`rotina ${schedule.name} ${schedule.cron_expression}`}
                  onSelect={() => run(() => navigate("/routines"))}
                  className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[12px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
                >
                  <CalendarClock size={14} className="shrink-0 text-zinc-500" />
                  <span className="min-w-0 flex-1 truncate">{schedule.name}</span>
                  <span className="shrink-0 font-mono text-[9px] text-zinc-500">
                    {schedule.is_active ? "ativa" : "pausada"}
                  </span>
                </Command.Item>
              ))}
            </Command.Group>
          ) : null}

          {artifacts.length > 0 ? (
            <Command.Group
              heading="Artefatos"
              className="mt-2 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[9px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-500"
            >
              {artifacts.slice(0, 10).map((artifact) => (
                <Command.Item
                  key={artifact.id}
                  value={`artefato ${artifact.file_path} ${artifact.created_by_agent}`}
                  onSelect={() =>
                    run(() =>
                      navigate(`/artifacts?id=${encodeURIComponent(artifact.id)}`),
                    )
                  }
                  className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[12px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
                >
                  <Archive size={14} className="shrink-0 text-zinc-500" />
                  <span className="min-w-0 flex-1 truncate font-mono text-[10px]">
                    {artifact.file_path}
                  </span>
                  <span className="shrink-0 text-[9px] text-zinc-500">
                    {relativeTime(artifact.created_at)}
                  </span>
                </Command.Item>
              ))}
            </Command.Group>
          ) : null}

          {agents.length > 0 ? (
            <Command.Group
              heading="Agentes"
              className="mt-2 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[9px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-500"
            >
              {agents.slice(0, 10).map((agent) => (
                <Command.Item
                  key={agent.id}
                  value={`agente ${agent.id} ${agent.name ?? ""}`}
                  onSelect={() => run(() => navigate("/studio"))}
                  className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[12px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
                >
                  <Wand2 size={14} className="shrink-0 text-zinc-500" />
                  <span className="min-w-0 flex-1 truncate">@{agent.id}</span>
                  <span className="shrink-0 text-[9px] text-zinc-500">
                    {agent.name ?? "agente"}
                  </span>
                </Command.Item>
              ))}
            </Command.Group>
          ) : null}

          <Command.Group
            heading="Ações"
            className="mt-2 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[9px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-500"
          >
            <Command.Item
              value="Nova tarefa"
              onSelect={() =>
                run(() => {
                  navigate("/kanban");
                  setTaskDialogOpen(true);
                })
              }
              className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
            >
              <KanbanSquare size={15} className="text-zinc-500" />
              Nova tarefa
            </Command.Item>
            <Command.Item
              value="Abrir execuções"
              onSelect={() => run(() => navigate("/runs"))}
              className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
            >
              <Activity size={15} className="text-zinc-500" />
              Abrir execuções
            </Command.Item>
            <Command.Item
              value="Novo workspace"
              onSelect={() =>
                run(() => setWorkspaceDialogOpen(true))
              }
              className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
            >
              <Plus size={15} className="text-zinc-500" />
              Novo workspace
            </Command.Item>
            <Command.Item
              value="Reiniciar OpenCode Daemon"
              onSelect={() => run(() => void restartDaemon())}
              className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
            >
              <RotateCw size={15} className="text-zinc-500" />
              Reiniciar OpenCode Daemon
            </Command.Item>
            <Command.Item
              value="Atualizar dados"
              onSelect={() => run(() => void refresh())}
              className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
            >
              <RefreshCw size={15} className="text-zinc-500" />
              Atualizar dados
            </Command.Item>
          </Command.Group>
        </Command.List>

        <div className="flex items-center justify-between border-t border-zinc-800/70 px-3 py-2 font-mono text-[9px] text-zinc-500">
          <span>↑↓ navegar · ↵ selecionar</span>
          <span>OpenDesk</span>
        </div>
      </Command>
    </div>
  );
}
