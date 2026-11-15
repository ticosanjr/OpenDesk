import { useEffect, useState } from "react";
import { ArrowLeft, Download, Loader2 } from "lucide-react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { PageBody, PageBodyFill, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { RunLogView } from "@/components/runs/RunLogView";
import { apiGet } from "@/lib/api";
import { downloadRun, type ExportFormat } from "@/lib/download";
import { links } from "@/lib/links";
import type { TaskRun } from "@/lib/types";
import { toast } from "@/store/toast-store";

export function RunDetailPage() {
  const { runId } = useParams<{ runId: string }>();
  const navigate = useNavigate();
  const [exporting, setExporting] = useState<ExportFormat | null>(null);
  const [run, setRun] = useState<TaskRun | null>(null);

  useEffect(() => {
    if (!runId) {
      setRun(null);
      return;
    }
    let cancelled = false;
    void apiGet<TaskRun>(`/api/runs/${encodeURIComponent(runId)}`)
      .then((value) => {
        if (!cancelled) setRun(value);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [runId]);

  async function exportRun(format: ExportFormat) {
    if (!runId) return;
    setExporting(format);
    try {
      await downloadRun(runId, format);
      toast({
        tone: "success",
        title: `Export ${format.toUpperCase()} gerado`,
        description: "O download foi iniciado.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível exportar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setExporting(null);
    }
  }
  if (!runId) {
    return (
      <PageBody>
        <PageHeader title="Execução" />
      </PageBody>
    );
  }
  return (
    <PageBodyFill>
      <div className="shrink-0">
        <PageHeader
          title="Execução"
          description={`ID ${runId.slice(0, 12)}… · sessão, tentativas, ferramentas e resultado.`}
          actions={
            <>
              {(["md", "html", "json"] as const).map((format) => (
                <Button
                  key={format}
                  variant="outline"
                  size="sm"
                  disabled={exporting !== null}
                  onClick={() => void exportRun(format)}
                  title={`Exportar sessão em ${format.toUpperCase()}`}
                >
                  {exporting === format ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    <Download size={13} />
                  )}
                  {format.toUpperCase()}
                </Button>
              ))}
              <Button variant="outline" size="sm" onClick={() => navigate("/runs")}>
                <ArrowLeft size={13} />
                Todas as execuções
              </Button>
            </>
          }
        />
        {run?.source === "chat" ? (
          <div className="mt-2 flex flex-wrap items-center gap-2 text-[12px] text-zinc-500">
            <Badge variant="blue">chat · {run.channel ?? "desktop"}</Badge>
            {run.origin_ref ? (
              <Link
                to={links.chat(run.origin_ref)}
                className="underline decoration-zinc-700 underline-offset-2 hover:text-zinc-300"
              >
                abrir conversa
              </Link>
            ) : null}
          </div>
        ) : null}
      </div>
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-zinc-800/80 bg-zinc-900/25">
        <RunLogView runId={runId} />
      </div>
    </PageBodyFill>
  );
}
