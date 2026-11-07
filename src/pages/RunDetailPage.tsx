import { ArrowLeft } from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import { PageBody, PageBodyFill, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { RunLogView } from "@/components/runs/RunLogView";

export function RunDetailPage() {
  const { runId } = useParams<{ runId: string }>();
  const navigate = useNavigate();
  if (!runId) {
    return (
      <PageBody>
        <PageHeader eyebrow="auditoria" title="Execução" />
      </PageBody>
    );
  }
  return (
    <PageBodyFill>
      <div className="shrink-0">
        <PageHeader
          eyebrow="auditoria"
          title="Execução"
          description={`run ${runId}`}
          actions={
            <Button variant="outline" size="sm" onClick={() => navigate("/runs")}>
              <ArrowLeft size={13} />
              Todas as execuções
            </Button>
          }
        />
      </div>
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-zinc-800/80 bg-zinc-900/25">
        <RunLogView runId={runId} />
      </div>
    </PageBodyFill>
  );
}
