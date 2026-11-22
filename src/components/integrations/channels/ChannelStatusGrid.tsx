import { KeyRound } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ChannelStatus } from "./types";

export function ChannelStatusGrid({
  statuses,
  revealed,
  busy,
  onGenerateToken,
}: {
  statuses: ChannelStatus[];
  revealed: { channel: string; token: string } | null;
  busy: string | null;
  onGenerateToken: (channel: string) => void;
}) {
  return (
    <>
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {statuses.map((status) => (
          <div
            key={status.channel}
            className="space-y-1 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
          >
            <div className="flex items-center justify-between gap-2">
              <span className="font-mono text-[12.5px] text-zinc-200">
                {status.channel}
              </span>
              <Badge variant={status.webhook_token_set ? "green" : "default"}>
                {status.webhook_token_set ? "token" : "sem token"}
              </Badge>
            </div>
            <p className="text-[11.5px] text-zinc-500">
              {status.connector} · {status.routes} rota(s)
            </p>
            <div className="flex items-center justify-between gap-2">
              <code className="truncate font-mono text-[10.5px] text-zinc-600">
                {status.webhook_path}
              </code>
              <Button
                variant="outline"
                size="sm"
                className="h-6 px-2 text-[11px]"
                disabled={busy === `token:${status.channel}`}
                onClick={() => onGenerateToken(status.channel)}
              >
                <KeyRound size={10} />
                Token
              </Button>
            </div>
          </div>
        ))}
      </div>

      {revealed ? (
        <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-[12px] text-amber-200">
          Token de <strong>{revealed.channel}</strong> (copie agora):{" "}
          <code className="break-all font-mono">{revealed.token}</code>
          <p className="mt-1 text-[11.5px] text-amber-200/70">
            Envie em <code className="font-mono">x-opendesk-channel-token</code>.
          </p>
        </div>
      ) : null}
    </>
  );
}
