import { ArrowLeft } from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
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
    <PageBody>
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
      <div className="h-[calc(100vh-9.5rem)] overflow-hidden rounded-xl border border-zinc-800/80 bg-zinc-900/25">
        <RunLogView runId={runId} />
      </div>
    </PageBody>
  );
}
