import { ExternalLink, FileCheck2 } from "lucide-react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { DismissButton, Row } from "@/components/dashboard/attention/Row";
import { Section } from "@/components/dashboard/attention/Section";
import type { Attention } from "@/components/dashboard/attention/types";
import { links } from "@/lib/links";

export function ApprovalsSection({
  tasks,
  busy,
  onApprove,
  onDismiss,
}: {
  tasks: Attention["awaiting_approval"];
  busy: string | null;
  onApprove: (taskId: string) => void;
  onDismiss: (taskId: string, version?: string | null) => void;
}) {
  if (tasks.length === 0) return null;
  return (
    <Section
      title="Planos aguardando aprovação"
      count={tasks.length}
      tone="amber"
    >
      {tasks.map((task) => (
        <Row
          key={`approval-${task.id}`}
          tone="amber"
          icon={<FileCheck2 size={13} />}
          title={task.title}
          meta="plano pronto para revisão"
          detail={task.plan_excerpt}
          actions={
            <>
              <Button
                size="xs"
                title="Aprovar o plano e iniciar a fase de Build"
                disabled={busy === `approve-${task.id}`}
                onClick={() => onApprove(task.id)}
              >
                Aprovar e iniciar Build
              </Button>
              <Link
                to={links.task(task.id)}
                className="inline-flex items-center gap-1 text-[13px] text-zinc-400 hover:text-zinc-200"
              >
                <ExternalLink size={10} />
                abrir
              </Link>
              <DismissButton
                label="Dispensar alerta de aprovação"
                onClick={() => onDismiss(task.id, task.version)}
              />
            </>
          }
        />
      ))}
    </Section>
  );
}
