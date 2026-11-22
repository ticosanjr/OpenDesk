import { GitBranch, GitMerge, Loader2, Trash2 } from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { relativeTime } from "@/lib/format";
import type { WorktreeEntry } from "@/lib/types";
import { worktreeTone } from "./worktrees-shared";

export function WorktreeTable({
  loading,
  worktrees,
  busy,
  onDiff,
  onMerge,
  onDiscard,
}: {
  loading: boolean;
  worktrees: WorktreeEntry[];
  busy: string | null;
  onDiff: (entry: WorktreeEntry) => void;
  onMerge: (entry: WorktreeEntry) => void;
  onDiscard: (entry: WorktreeEntry) => void;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Execuções isoladas</CardTitle>
        <p className="text-[13px] text-zinc-500">
          Cada worktree pertence a um run. Merge exige aprovação; descartar remove
          a cópia isolada.
        </p>
      </CardHeader>
      <CardContent className="p-0">
        {loading && worktrees.length === 0 ? (
          <LoadingState label="Carregando worktrees…" />
        ) : worktrees.length === 0 ? (
          <EmptyState
            icon={GitBranch}
            title="Nenhuma worktree"
            description="Ative runs isolados em Configurações ou crie um A/B para gerar duas worktrees."
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[820px] border-collapse text-left text-[13px]">
              <thead className="border-b border-zinc-800/70 text-[12px] uppercase tracking-wide text-zinc-500">
                <tr>
                  <th className="px-4 py-2 font-medium">Execução</th>
                  <th className="px-4 py-2 font-medium">Isolamento</th>
                  <th className="px-4 py-2 font-medium">Diff</th>
                  <th className="px-4 py-2 font-medium">Status</th>
                  <th className="px-4 py-2 font-medium">Criada</th>
                  <th className="px-4 py-2 text-right font-medium">Ações</th>
                </tr>
              </thead>
              <tbody>
                {worktrees.map((entry) => (
                  <tr
                    key={entry.id}
                    className="border-b border-zinc-800/50 align-top last:border-0"
                  >
                    <td className="px-4 py-3">
                      <div className="font-medium text-zinc-200">
                        {entry.task_title ?? "Execução sem tarefa"}
                      </div>
                      {entry.run_id ? (
                        <Link
                          to={`/runs/${encodeURIComponent(entry.run_id)}`}
                          className="font-mono text-[12px] text-blue-300 hover:underline"
                        >
                          run {entry.run_id.slice(0, 12)}…
                        </Link>
                      ) : null}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-2 text-zinc-300">
                        <GitBranch size={13} className="text-zinc-500" />
                        {entry.mode}
                      </div>
                      {entry.branch ? (
                        <div className="mt-1 font-mono text-[12px] text-zinc-500">
                          {entry.branch}
                        </div>
                      ) : null}
                    </td>
                    <td className="px-4 py-3 font-mono text-[12px]">
                      <span className="text-zinc-400">{entry.diff_stat.files} arq.</span>{" "}
                      <span className="text-emerald-300">
                        +{entry.diff_stat.insertions}
                      </span>{" "}
                      <span className="text-red-300">
                        -{entry.diff_stat.deletions}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <Badge variant={worktreeTone(entry.status)}>{entry.status}</Badge>
                    </td>
                    <td className="px-4 py-3 text-zinc-500">
                      {relativeTime(entry.created_at)}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex justify-end gap-1.5">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => onDiff(entry)}
                          disabled={busy === `diff:${entry.id}`}
                        >
                          {busy === `diff:${entry.id}` ? (
                            <Loader2 size={12} className="animate-spin" />
                          ) : null}
                          Diff
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => onMerge(entry)}
                          disabled={entry.status !== "ACTIVE" || busy === `merge:${entry.id}`}
                        >
                          <GitMerge size={12} />
                          Mesclar
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="text-red-300 hover:text-red-200"
                          onClick={() => onDiscard(entry)}
                          disabled={entry.status !== "ACTIVE" || busy === `discard:${entry.id}`}
                        >
                          <Trash2 size={12} />
                          Descartar
                        </Button>
                      </div>
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
