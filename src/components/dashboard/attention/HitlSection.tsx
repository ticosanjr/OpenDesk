import { HitlCard } from "@/components/dashboard/attention/HitlCard";
import { Section } from "@/components/dashboard/attention/Section";
import type { HitlRequest } from "@/components/dashboard/attention/types";

export function HitlSection({
  requests,
  busy,
  feedbackToken,
  feedbackText,
  onFeedbackText,
  onToggleFeedback,
  onApprove,
  onReject,
  onChoose,
  onFeedback,
}: {
  requests: HitlRequest[];
  busy: string | null;
  feedbackToken: string | null;
  feedbackText: string;
  onFeedbackText: (value: string) => void;
  onToggleFeedback: (token: string) => void;
  onApprove: (request: HitlRequest) => void;
  onReject: (request: HitlRequest) => void;
  onChoose: (request: HitlRequest, index: number, value: string) => void;
  onFeedback: (request: HitlRequest) => void;
}) {
  if (requests.length === 0) return null;
  return (
    <Section title="Checkpoints de receita" count={requests.length} tone="amber">
      <div className="space-y-2">
        {requests.map((request) => (
          <HitlCard
            key={`hitl-${request.token}`}
            request={request}
            busy={busy === `hitl-${request.token}`}
            feedbackOpen={feedbackToken === request.token}
            feedbackText={feedbackText}
            onFeedbackText={onFeedbackText}
            onToggleFeedback={() => onToggleFeedback(request.token)}
            onApprove={() => onApprove(request)}
            onReject={() => onReject(request)}
            onChoose={(index, value) => onChoose(request, index, value)}
            onFeedback={() => onFeedback(request)}
          />
        ))}
      </div>
    </Section>
  );
}
