import { useCallback, useEffect, useState } from "react";
import { Inbox, Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiGet, apiPatch } from "@/lib/api";
import { relativeTime } from "@/lib/format";
import { toast } from "@/store/toast-store";

interface Ticket {
  id: string;
  source: string;
  title: string;
  body: string;
  priority: number;
  status: string;
  assigned_agent?: string | null;
  task_id?: string | null;
  created_at: string;
}

const STATUSES = ["OPEN", "TRIAGED", "IN_PROGRESS", "RESOLVED", "DISMISSED"];

export function TicketInboxCard({ workspaceId }: { workspaceId: string }) {
  const [tickets, setTickets] = useState<Ticket[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      setTickets(
        await apiGet<Ticket[]>(
          `/api/tickets?workspace_id=${encodeURIComponent(workspaceId)}&limit=50`,
        ),
      );
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar os tickets",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function changeStatus(ticket: Ticket, status: string) {
    setBusy(ticket.id);
    try {
      await apiPatch(`/api/tickets/${ticket.id}`, { status });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao atualizar o ticket",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  return (
    <Card id="integrations-tickets">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Inbox size={15} className="text-zinc-500" />
            Caixa de entrada
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Tudo que chega por canal vira ticket. Identidades não confiáveis param
            aqui; as demais também geram tarefa (eventualmente retida).
          </p>
        </div>
      </CardHeader>
      <CardContent>
        {loading ? (
          <div className="flex items-center gap-2 py-3 text-[12.5px] text-zinc-500">
            <Loader2 size={13} className="animate-spin" />
            carregando tickets…
          </div>
        ) : tickets.length === 0 ? (
          <EmptyState
            title="Nenhum ticket"
            description="Mensagens recebidas pelos canais aparecem aqui."
          />
        ) : (
          <div className="space-y-2">
            {tickets.map((ticket) => (
              <div
                key={ticket.id}
                className="flex flex-wrap items-start justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
              >
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <Badge variant={ticket.priority >= 3 ? "yellow" : "default"}>
                      {ticket.source}
                    </Badge>
                    <span className="text-[13px] font-medium text-zinc-200">
                      {ticket.title}
                    </span>
                    {ticket.task_id ? <Badge variant="blue">tarefa</Badge> : null}
                  </div>
                  <p className="mt-0.5 line-clamp-2 text-[12px] text-zinc-500">
                    {ticket.body}
                  </p>
                  <p className="mt-0.5 text-[11px] text-zinc-600">
                    {ticket.assigned_agent ? `@${ticket.assigned_agent} · ` : ""}
                    {relativeTime(ticket.created_at)}
                  </p>
                </div>
                <select
                  value={ticket.status}
                  disabled={busy === ticket.id}
                  onChange={(event) => void changeStatus(ticket, event.target.value)}
                  className="h-7 rounded border border-zinc-700 bg-zinc-950 px-2 text-[12px] text-zinc-300"
                >
                  {STATUSES.map((status) => (
                    <option key={status} value={status}>
                      {status}
                    </option>
                  ))}
                </select>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
