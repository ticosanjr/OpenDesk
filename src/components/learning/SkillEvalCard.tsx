import { useCallback, useEffect, useState } from "react";
import { BarChart3, Loader2, RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiGet } from "@/lib/api";
import { relativeTime } from "@/lib/format";
import { toast } from "@/store/toast-store";

interface SkillUsage {
  id: string;
  name: string;
  description: string;
  mentions: number;
  successes: number;
  failures: number;
  success_rate?: number | null;
  last_used_at?: string | null;
}

export function SkillEvalCard({ workspaceId }: { workspaceId: string }) {
  const [skills, setSkills] = useState<SkillUsage[]>([]);
  const [days, setDays] = useState(30);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      setSkills(
        await apiGet<SkillUsage[]>(
          `/api/skills/eval?workspace_id=${encodeURIComponent(workspaceId)}&days=${days}`,
        ),
      );
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível avaliar as skills",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [workspaceId, days]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <BarChart3 size={15} className="text-zinc-500" />
            Avaliação de skills
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Menções nos logs de execução por skill nativa do workspace. Serve
            para aposentar skills sem uso e investigar falhas recorrentes.
          </p>
        </div>
        <div className="flex items-center gap-1.5">
          <select
            value={days}
            onChange={(event) => setDays(Number(event.target.value))}
            className="h-7 rounded border border-zinc-700 bg-zinc-950 px-2 text-[12px] text-zinc-300"
          >
            <option value={7}>7 dias</option>
            <option value={30}>30 dias</option>
            <option value={90}>90 dias</option>
          </select>
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={() => void load()}
            disabled={loading}
          >
            <RefreshCw size={11} className={loading ? "animate-spin" : undefined} />
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        {loading ? (
          <div className="flex items-center gap-2 text-[12.5px] text-zinc-500">
            <Loader2 size={13} className="animate-spin" />
            avaliando…
          </div>
        ) : skills.length === 0 ? (
          <EmptyState
            title="Nenhuma skill no workspace"
            description="Skills nativas ficam em .opencode/skills/*/SKILL.md."
          />
        ) : (
          <div className="overflow-hidden rounded-lg border border-zinc-800/70">
            <table className="w-full text-left text-[12.5px]">
              <thead className="bg-zinc-900/60 text-[11px] uppercase tracking-[0.08em] text-zinc-500">
                <tr>
                  <th className="px-3 py-1.5">skill</th>
                  <th className="px-3 py-1.5">menções</th>
                  <th className="px-3 py-1.5">sucesso</th>
                  <th className="px-3 py-1.5">último uso</th>
                </tr>
              </thead>
              <tbody>
                {skills.map((skill) => (
                  <tr key={skill.id} className="border-t border-zinc-800/60">
                    <td className="px-3 py-1.5">
                      <div className="flex items-center gap-1.5">
                        <span className="font-mono text-zinc-300">{skill.name}</span>
                        {skill.mentions === 0 ? (
                          <Badge variant="default">sem uso</Badge>
                        ) : null}
                      </div>
                      <p className="max-w-md truncate text-[11px] text-zinc-600">
                        {skill.description}
                      </p>
                    </td>
                    <td className="px-3 py-1.5 text-zinc-400">{skill.mentions}</td>
                    <td className="px-3 py-1.5">
                      {skill.success_rate == null ? (
                        <span className="text-zinc-600">—</span>
                      ) : (
                        <Badge
                          variant={
                            skill.success_rate >= 0.8
                              ? "green"
                              : skill.success_rate >= 0.5
                                ? "yellow"
                                : "red"
                          }
                        >
                          {(skill.success_rate * 100).toFixed(0)}%
                        </Badge>
                      )}
                    </td>
                    <td className="px-3 py-1.5 text-zinc-500">
                      {skill.last_used_at ? relativeTime(skill.last_used_at) : "—"}
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
