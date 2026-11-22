import { Loader2, ShieldCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { formatDuration, formatNumber, formatUsd } from "@/lib/format";
import type { AbDetail } from "@/lib/types";
import { abTone } from "./worktrees-shared";

export function AbDetailCard({
  detail,
  busy,
  onChoose,
  onShowDiff,
}: {
  detail: AbDetail;
  busy: string | null;
  onChoose: (slot: string) => void;
  onShowDiff: (title: string, patch: string) => void;
}) {
  return (
    <Card>
      <CardHeader className="flex-row items-start justify-between gap-3">
        <div>
          <CardTitle>{detail.title}</CardTitle>
          <p className="mt-1 text-[13px] text-zinc-500">{detail.prompt}</p>
        </div>
        <Badge variant={abTone(detail.status)}>{detail.status}</Badge>
      </CardHeader>
      <CardContent>
        <div className="grid gap-3 md:grid-cols-2">
          {detail.slots.map((slot) => (
            <div
              key={slot.slot}
              className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="font-mono text-[13px] font-semibold text-zinc-200">
                  SLOT {slot.slot.toUpperCase()}
                </span>
                <Badge variant={slot.status === "SUCCESS" ? "green" : "blue"}>
                  {slot.status}
                </Badge>
              </div>
              <div className="mt-2 space-y-1 text-[12.5px] text-zinc-400">
                <p>
                  {slot.agent} · <span className="font-mono">{slot.model || "—"}</span>
                </p>
                <p className="font-mono">
                  {formatUsd(slot.cost_usd)} · {formatNumber(slot.tokens)} tokens ·{" "}
                  {formatDuration(slot.duration_ms)}
                </p>
                <p className="font-mono">
                  {slot.diff_stat.files} arq.{" "}
                  <span className="text-emerald-300">+{slot.diff_stat.insertions}</span>{" "}
                  <span className="text-red-300">-{slot.diff_stat.deletions}</span>
                </p>
              </div>
              <div className="mt-3 flex flex-wrap gap-1.5">
                {slot.diff ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() =>
                      onShowDiff(`Slot ${slot.slot.toUpperCase()}`, slot.diff ?? "")
                    }
                  >
                    Ver diff
                  </Button>
                ) : null}
                <Button
                  size="sm"
                  variant={detail.winner_slot === slot.slot ? "default" : "outline"}
                  disabled={
                    detail.status !== "OPEN" ||
                    slot.status !== "SUCCESS" ||
                    busy === `choose:${slot.slot}`
                  }
                  onClick={() => onChoose(slot.slot)}
                >
                  {busy === `choose:${slot.slot}` ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <ShieldCheck size={12} />
                  )}
                  Escolher {slot.slot.toUpperCase()}
                </Button>
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
