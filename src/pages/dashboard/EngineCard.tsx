import { Server } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { InfoRow } from "@/components/ui/info-row";
import { StatusDot, statusTone } from "@/components/ui/status-dot";
import type { DaemonStatus } from "@/lib/types";

export function EngineCard({ daemon }: { daemon: DaemonStatus | null }) {
  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Server size={15} className="text-zinc-500" />
            OpenCode engine
          </CardTitle>
          <CardDescription>Daemon gerenciado pelo OpenDesk.</CardDescription>
        </div>
        <StatusDot
          tone={statusTone(daemon?.online ?? false, daemon?.state)}
        />
      </CardHeader>
      <CardContent className="space-y-3">
        <InfoRow
          label="Estado"
          value={daemon?.online ? "online" : daemon?.state ?? "offline"}
        />
        <InfoRow label="Porta" value={daemon ? String(daemon.port) : "—"} />
        <InfoRow
          label="Versão"
          value={daemon?.version ? `v${daemon.version}` : "—"}
        />
        <InfoRow
          label="Jobs ativos"
          value={String(daemon?.active_jobs ?? 0)}
        />
        {daemon?.message ? (
          <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-2.5 py-2 text-[12px] leading-relaxed text-amber-300">
            {daemon.message}
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}
