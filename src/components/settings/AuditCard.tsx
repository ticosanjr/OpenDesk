import { useCallback, useEffect, useMemo, useState } from "react";
import { RefreshCw, Search, ShieldCheck } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { apiGet } from "@/lib/api";
import { relativeTime } from "@/lib/format";
import type { AuditEntry, AuditResponse } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { cn } from "@/lib/utils";

function statusTone(status: number): "green" | "yellow" | "red" {
  if (status < 300) return "green";
  if (status < 500) return "yellow";
  return "red";
}

export function AuditCard() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [data, setData] = useState<AuditResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const suffix = activeWorkspaceId
        ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}&limit=300`
        : "?limit=300";
      setData(await apiGet<AuditResponse>(`/api/audit${suffix}`));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  const entries = useMemo(() => {
    const rows = data?.entries ?? [];
    const needle = query.trim().toLowerCase();
    if (!needle) return rows;
    return rows.filter((entry) =>
      `${entry.action} ${entry.method} ${entry.path} ${entry.status}`
        .toLowerCase()
        .includes(needle),
    );
  }, [data, query]);

  return (
    <Card id="settings-audit">
      <CardHeader className="flex flex-row items-start justify-between gap-3">
        <div>
          <CardTitle className="flex items-center gap-2">
            <ShieldCheck size={15} className="text-zinc-400" />
            Auditoria da API local
          </CardTitle>
          <p className="mt-1 text-xs leading-relaxed text-zinc-500">
            Mutações e leituras sensíveis são registradas sem corpo de requisição,
            prompts ou credenciais. O rate limit protege a API contra rajadas e
            loops de agentes.
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
          <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
          Atualizar
        </Button>
      </CardHeader>
      <CardContent className="space-y-3">
        {data ? (
          <div className="flex flex-wrap gap-2 text-[11.5px]">
            <Badge variant="default">leitura {data.rate_limits.read_per_minute}/min</Badge>
            <Badge variant="default">escrita {data.rate_limits.write_per_minute}/min</Badge>
            <Badge variant="default">
              sensível {data.rate_limits.sensitive_per_minute}/min
            </Badge>
          </div>
        ) : null}
        <div className="flex h-8 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5">
          <Search size={13} className="shrink-0 text-zinc-500" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Filtrar por ação, método, rota ou status…"
            className="h-full min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-zinc-500"
          />
        </div>
        {loading && !data ? (
          <LoadingState rows={3} label="Carregando auditoria…" />
        ) : entries.length === 0 ? (
          <EmptyState
            icon={ShieldCheck}
            title="Nenhuma chamada auditada"
            description="Crie um backup, edite um agente ou execute uma tarefa para gerar registros."
          />
        ) : (
          <div className="overflow-x-auto rounded-lg border border-zinc-800/70">
            <table className="w-full text-left text-[12px]">
              <thead className="border-b border-zinc-800 text-[11.5px] uppercase tracking-wide text-zinc-500">
                <tr>
                  <th className="px-3 py-2">Quando</th>
                  <th className="px-3 py-2">Ação</th>
                  <th className="px-3 py-2">Rota</th>
                  <th className="px-3 py-2">Status</th>
                  <th className="px-3 py-2">Duração</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((entry: AuditEntry) => (
                  <tr
                    key={entry.id}
                    className="border-b border-zinc-800/50 last:border-0"
                  >
                    <td className="whitespace-nowrap px-3 py-2 text-zinc-500">
                      {relativeTime(entry.created_at)}
                    </td>
                    <td className="px-3 py-2 font-mono text-zinc-300">
                      {entry.action}
                    </td>
                    <td className="max-w-[320px] truncate px-3 py-2 font-mono text-zinc-500">
                      {entry.method} {entry.path}
                    </td>
                    <td className="px-3 py-2">
                      <Badge variant={statusTone(entry.status)}>
                        {entry.status}
                      </Badge>
                    </td>
                    <td
                      className={cn(
                        "px-3 py-2 text-zinc-500",
                        entry.duration_ms > 2000 && "text-amber-300",
                      )}
                    >
                      {entry.duration_ms} ms
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
