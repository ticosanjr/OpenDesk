import { Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Row } from "@/components/dashboard/attention/Row";
import { Section } from "@/components/dashboard/attention/Section";
import type { Proposal } from "@/components/dashboard/attention/types";

export function ProposalsSection({
  proposals,
  busy,
  onResolve,
}: {
  proposals: Proposal[];
  busy: string | null;
  onResolve: (proposal: Proposal, approved: boolean) => void;
}) {
  if (proposals.length === 0) return null;
  return (
    <Section title="Propostas de configuração" count={proposals.length} tone="violet">
      {proposals.map((proposal) => (
        <Row
          key={`proposal-${proposal.id}`}
          tone="violet"
          icon={<Sparkles size={13} />}
          title={proposal.title}
          meta={`${proposal.kind} · ${proposal.action}${
            proposal.requested_by ? ` · @${proposal.requested_by}` : ""
          }`}
          detail={proposal.apply_error ?? proposal.description}
          actions={
            <>
              <Button
                size="xs"
                disabled={busy === `proposal-${proposal.id}`}
                onClick={() => onResolve(proposal, true)}
              >
                Aprovar
              </Button>
              <Button
                size="xs"
                variant="outline"
                className="border-red-500/30 text-red-300"
                disabled={busy === `proposal-${proposal.id}`}
                onClick={() => onResolve(proposal, false)}
              >
                Negar
              </Button>
            </>
          }
        />
      ))}
    </Section>
  );
}
