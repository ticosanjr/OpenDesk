import { Loader2, RefreshCw, Send } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import type { Delivery } from "@/lib/types";
import { relativeTime } from "@/lib/format";

export function DeliveriesCard({
  deliveries,
  retrying,
  onRetry,
}: {
  deliveries: Delivery[];
  retrying: string | null;
  onRetry: (id: string) => void;
}) {
  return (
    <Card id="integrations-deliveries">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Send size={15} className="text-zinc-500" />
            Entregas recentes
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Idempotência por run + destinatário em integration_deliveries.
          </p>
        </div>
        <Badge variant="default">{deliveries.length}</Badge>
      </CardHeader>
      <CardContent>
        {deliveries.length === 0 ? (
          <EmptyState
            icon={Send}
            title="Nenhuma entrega ainda"
            description="Ao concluir um run com notify_email, o OpenDesk envia o resumo e os anexos e registra o status aqui."
          />
        ) : (
          <div className="space-y-2">
            {deliveries.map((delivery) => (
              <div
                key={delivery.id}
                className="flex flex-wrap items-center gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
              >
                <Badge
                  variant={
                    delivery.status === "SUCCESS"
                      ? "green"
                      : delivery.status === "FAILED"
                        ? "red"
                        : "default"
                  }
                >
                  {delivery.status}
                </Badge>
                <span className="font-mono text-[12px] text-zinc-300">{delivery.channel}</span>
                <span className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-zinc-500">
                  {delivery.idempotency_key}
                </span>
                <span className="font-mono text-[11.5px] text-zinc-500">
                  {delivery.attempts} tentativa(s) · {relativeTime(delivery.updated_at)}
                </span>
                {delivery.status === "FAILED" ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => onRetry(delivery.id)}
                    disabled={retrying === delivery.id}
                    aria-label={`Reenviar entrega ${delivery.id}`}
                  >
                    {retrying === delivery.id ? (
                      <Loader2 size={12} className="animate-spin" />
                    ) : (
                      <RefreshCw size={12} />
                    )}
                    Reenviar
                  </Button>
                ) : null}
                {delivery.last_error ? (
                  <span className="w-full truncate text-[11.5px] text-red-300">
                    {delivery.last_error}
                  </span>
                ) : null}
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
