import { ArrowUpRight, CheckCircle2, Sparkles, XCircle } from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { proposalSkillId } from "@/lib/learning";
import { links } from "@/lib/links";
import type { LearningProposal } from "@/lib/types";

export function LearningProposalsSection({
  proposals,
  busy,
  workspaceId,
  onResolve,
}: {
  proposals: LearningProposal[];
  busy: string | null;
  workspaceId: string;
  onResolve: (proposal: LearningProposal, approved: boolean) => void;
}) {
  return (
    <section className="space-y-2">
      <div className="flex items-center gap-2">
        <Sparkles size={13} className="text-violet-300" />
        <h2 className="text-[13.5px] font-medium text-zinc-200">
          Propostas aguardando aprovação
        </h2>
        <Badge variant="purple">{proposals.length}</Badge>
      </div>
      <div className="space-y-1.5">
        {proposals.map((proposal) => {
          const skillId = proposalSkillId(proposal);
          return (
            <Card key={proposal.id} className="border-violet-500/20">
              <CardContent className="flex flex-wrap items-center gap-3 p-3">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <Badge variant="purple">{proposal.kind}</Badge>
                    <Badge variant="default">{proposal.action}</Badge>
                    <p className="truncate text-[13px] text-zinc-200">
                      {proposal.title}
                    </p>
                  </div>
                  <p className="mt-1 text-[11.5px] text-zinc-500">
                    {proposal.description ??
                      "Revise antes de gravar no workspace."}
                  </p>
                </div>
                <div className="flex items-center gap-1.5">
                  {skillId ? (
                    <Link
                      to={links.skill(skillId, "workspace", workspaceId)}
                      className="inline-flex h-8 items-center gap-1 rounded-lg border border-zinc-800 px-2.5 text-[12px] text-zinc-300 hover:bg-zinc-900"
                    >
                      Ver skill
                      <ArrowUpRight size={11} />
                    </Link>
                  ) : null}
                  <Button
                    size="sm"
                    disabled={busy === `proposal-${proposal.id}`}
                    onClick={() => onResolve(proposal, true)}
                  >
                    <CheckCircle2 size={12} />
                    Aprovar
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    className="border-red-500/30 text-red-300"
                    disabled={busy === `proposal-${proposal.id}`}
                    onClick={() => onResolve(proposal, false)}
                  >
                    <XCircle size={12} />
                    Rejeitar
                  </Button>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </section>
  );
}
