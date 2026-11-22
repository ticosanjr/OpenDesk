import { ExternalLink, ShieldAlert } from "lucide-react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Row } from "@/components/dashboard/attention/Row";
import { Section } from "@/components/dashboard/attention/Section";
import type { PermissionRequest } from "@/components/dashboard/attention/types";
import { links } from "@/lib/links";

export function PermissionsSection({
  requests,
  busy,
  onDecide,
}: {
  requests: PermissionRequest[];
  busy: string | null;
  onDecide: (request: PermissionRequest, decision: string) => void;
}) {
  if (requests.length === 0) return null;
  return (
    <Section title="Permissões" count={requests.length} tone="amber">
      {requests.map((request) => (
        <Row
          key={`permission-${request.request_id}`}
          tone="amber"
          icon={<ShieldAlert size={13} />}
          title={
            <span>
              <span className="font-mono text-[13px] uppercase text-amber-300">
                {request.permission_action}
              </span>{" "}
              {request.resources || "ação do agente"}
            </span>
          }
          meta={request.task_id ? "tarefa vinculada" : "sem tarefa"}
          detail="O agente pediu esta permissão para continuar. Permitir uma vez vale só para esta ação; permitir nesta tarefa cobre as próximas ações iguais; negar bloqueia o run."
          actions={
            <>
              <Button
                size="xs"
                title="Liberar apenas esta solicitação"
                disabled={busy === `permission-${request.request_id}`}
                onClick={() => onDecide(request, "once")}
              >
                Permitir uma vez
              </Button>
              <Button
                size="xs"
                variant="outline"
                title="Liberar ações iguais nesta tarefa"
                disabled={busy === `permission-${request.request_id}`}
                onClick={() => onDecide(request, "always")}
              >
                Nesta tarefa
              </Button>
              <Button
                size="xs"
                variant="outline"
                className="border-red-500/30 text-red-300"
                title="Bloquear a ação e interromper o run"
                disabled={busy === `permission-${request.request_id}`}
                onClick={() => onDecide(request, "reject")}
              >
                Negar
              </Button>
              {request.task_id ? (
                <Link
                  to={links.task(request.task_id)}
                  className="inline-flex items-center gap-1 text-[13px] text-zinc-400 hover:text-zinc-200"
                >
                  <ExternalLink size={10} />
                  abrir
                </Link>
              ) : null}
            </>
          }
        />
      ))}
    </Section>
  );
}
