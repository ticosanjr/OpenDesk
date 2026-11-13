import { useCallback, useEffect, useMemo, useState } from "react";
import { BookOpen, Bot, Plug, Terminal } from "lucide-react";
import { apiGet } from "@/lib/api";
import type { EffectiveStudioItem } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { CountPill } from "@/components/ui/count-pill";
import { LoadingState } from "@/components/ui/loading-state";

const KIND_META = [
  { kind: "agent" as const, label: "Agentes", icon: Bot },
  { kind: "skill" as const, label: "Skills", icon: BookOpen },
  { kind: "command" as const, label: "Comandos", icon: Terminal },
  { kind: "mcp" as const, label: "MCPs", icon: Plug },
];

export function StudioScopeSummary({
  scope,
  workspaceId,
}: {
  scope: "global" | "workspace" | "effective";
  workspaceId: string | null;
}) {
  const [items, setItems] = useState<EffectiveStudioItem[]>([]);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    if (!workspaceId) {
      setItems([]);
      return;
    }
    setLoading(true);
    try {
      const response = await apiGet<{ items: EffectiveStudioItem[] }>(
        `/api/studio/effective?workspace_id=${encodeURIComponent(workspaceId)}`,
      );
      setItems(response.items);
    } catch {
      setItems([]);
    } finally {
      setLoading(false);
    }
  }, [workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  const visible = useMemo(() => {
    if (scope === "effective") return items;
    return items.filter((item) => item.source_scope === scope);
  }, [items, scope]);

  const counts = useMemo(() => {
    const byKind = Object.fromEntries(
      KIND_META.map((meta) => [
        meta.kind,
        visible.filter((item) => item.kind === meta.kind).length,
      ]),
    ) as Record<(typeof KIND_META)[number]["kind"], number>;
    return {
      byKind,
      disabled: visible.filter((item) => item.disabled).length,
      overridden: visible.filter((item) => item.overridden).length,
    };
  }, [visible]);

  return (
    <Card className="border-zinc-800/80 bg-zinc-950/35">
      <CardContent className="space-y-3 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <p className="text-[13px] font-semibold uppercase tracking-[0.08em] text-zinc-300">
            Resumo do escopo
          </p>
          <Badge variant={scope === "global" ? "purple" : scope === "workspace" ? "blue" : "green"}>
            {scope === "global"
              ? "global"
              : scope === "workspace"
                ? "workspace"
                : "efetivo"}
          </Badge>
          <span className="ml-auto text-[11.5px] text-zinc-500">
            {visible.length} item(ns) · {counts.disabled} desabilitado(s) ·{" "}
            {counts.overridden} sobreposto(s)
          </span>
        </div>
        {loading && visible.length === 0 ? (
          <LoadingState compact rows={1} label="Carregando resumo…" />
        ) : (
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            {KIND_META.map((meta) => {
              const Icon = meta.icon;
              return (
                <div
                  key={meta.kind}
                  className="flex items-center gap-3 rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-3 py-2"
                >
                  <span className="flex h-7 w-7 items-center justify-center rounded-md border border-zinc-800 bg-zinc-950/60 text-zinc-400">
                    <Icon size={14} />
                  </span>
                  <div className="min-w-0">
                    <p className="truncate text-[11.5px] uppercase tracking-wide text-zinc-500">
                      {meta.label}
                    </p>
                    <p className="font-mono text-[17px] leading-tight text-zinc-100">
                      {counts.byKind[meta.kind]}
                    </p>
                  </div>
                  <CountPill
                    value={visible.filter((item) => item.kind === meta.kind && !item.disabled).length}
                    tone="green"
                  />
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
