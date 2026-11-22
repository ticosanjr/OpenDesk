import { Ban, ExternalLink, Trash2 } from "lucide-react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Tooltip } from "@/components/ui/tooltip";
import { DismissButton, Row } from "@/components/dashboard/attention/Row";
import { Section } from "@/components/dashboard/attention/Section";
import type { Attention } from "@/components/dashboard/attention/types";
import { links } from "@/lib/links";

export function BlockedSection({
  tasks,
  busy,
  onDismiss,
  onRemoveTask,
}: {
  tasks: Attention["blocked"];
  busy: string | null;
  onDismiss: (taskId: string, version?: string | null) => void;
  onRemoveTask: (taskId: string, title: string) => void;
}) {
  if (tasks.length === 0) return null;
  return (
    <Section title="Tarefas bloqueadas" count={tasks.length} tone="red">
      {tasks.map((task) => (
        <Row
          key={`blocked-${task.id}`}
          tone="red"
          icon={<Ban size={13} />}
          title={task.title}
          meta="bloqueada"
          detail={task.reason}
          actions={
            <>
              <Link
                to={links.task(task.id)}
                className="inline-flex items-center gap-1 text-[13px] text-zinc-400 hover:text-zinc-200"
              >
                <ExternalLink size={10} />
                resolver
              </Link>
              <Tooltip content="Remover tarefa">
                <Button
                  size="sm"
                  variant="outline"
                  className="h-7 border-red-500/30 px-2 text-red-300"
                  aria-label="Remover tarefa"
                  disabled={busy === `remove-${task.id}`}
                  onClick={() => onRemoveTask(task.id, task.title)}
                >
                  <Trash2 size={12} />
                </Button>
              </Tooltip>
              <DismissButton
                label="Dispensar alerta de bloqueio"
                onClick={() => onDismiss(task.id, task.version)}
              />
            </>
          }
        />
      ))}
    </Section>
  );
}
