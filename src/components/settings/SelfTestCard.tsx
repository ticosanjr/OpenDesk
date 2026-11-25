import { useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  Loader2,
  Play,
  Sparkles,
  XCircle,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiPost } from "@/lib/api";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";

type Mode = "fast" | "full_ai";
type Status = "pass" | "warn" | "fail" | "skipped";

interface SelfTestStep {
  id: string;
  label: string;
  status: Status;
  duration_ms: number;
  detail: string;
}

interface SelfTestReport {
  ok: boolean;
  mode: Mode;
  total_ms: number;
  steps: SelfTestStep[];
}

const ICONS: Record<Status, typeof CheckCircle2> = {
  pass: CheckCircle2,
  warn: AlertTriangle,
  fail: XCircle,
  skipped: XCircle,
};

const COLORS: Record<Status, string> = {
  pass: "text-emerald-300",
  warn: "text-amber-300",
  fail: "text-red-300",
  skipped: "text-zinc-500",
};

export function SelfTestCard({ workspaceId }: { workspaceId: string }) {
  const [report, setReport] = useState<SelfTestReport | null>(null);
  const [busy, setBusy] = useState<Mode | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function run(mode: Mode) {
    if (!workspaceId) return;
    if (mode === "full_ai") {
      const confirmed = await confirmAction({
        title: "Rodar o teste completo com IA?",
        description:
          "Cria uma tarefa e uma receita mínimas, executa com o motor e consome poucos tokens. Tudo é apagado ao final.",
        confirmLabel: "Testar com IA",
      });
      if (!confirmed) return;
    }
    setBusy(mode);
    setError(null);
    try {
      const value = await apiPost<SelfTestReport>("/api/diagnostics/self-test", {
        workspace_id: workspaceId,
        mode,
      });
      setReport(value);
      toast({
        tone: value.ok ? "success" : "warning",
        title: value.ok ? "Teste concluído" : "Teste encontrou falhas",
        description: `${value.steps.length} etapas · ${value.total_ms} ms`,
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function copyReport() {
    if (!report) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(report, null, 2));
      toast({ tone: "success", title: "Relatório copiado" });
    } catch {
      toast({ tone: "error", title: "Não foi possível copiar" });
    }
  }

  return (
    <Card id="settings-self-test" className="scroll-mt-14">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Sparkles size={15} className="text-zinc-500" />
            Testar tudo
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Verifica banco, motor, tarefa, rotina, receita, script MCP e voz. O
            modo rápido funciona offline e não consome tokens; o completo executa
            versões mínimas com IA.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Button
            size="sm"
            onClick={() => void run("fast")}
            disabled={!workspaceId || busy !== null}
          >
            {busy === "fast" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Play size={13} />
            )}
            Testar tudo (rápido)
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void run("full_ai")}
            disabled={!workspaceId || busy !== null}
            title="Consome poucos tokens: cria e executa uma tarefa/receita mínimas"
          >
            {busy === "full_ai" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Sparkles size={13} />
            )}
            Testar tudo com IA
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            {error}
          </p>
        ) : null}
        {report ? (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant={report.ok ? "green" : "red"}>
                {report.ok ? "aprovado" : "com falhas"}
              </Badge>
              <Badge variant="default">{report.mode}</Badge>
              <span className="font-mono text-[11.5px] text-zinc-500">
                {report.total_ms} ms
              </span>
              <Button
                variant="ghost"
                size="sm"
                className="ml-auto h-7"
                onClick={() => void copyReport()}
              >
                <Copy size={12} />
                copiar relatório
              </Button>
            </div>
            <div className="space-y-1">
              {report.steps.map((step) => {
                const Icon = ICONS[step.status];
                return (
                  <div
                    key={step.id}
                    className="flex items-start gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2"
                  >
                    <Icon
                      size={14}
                      className={`mt-0.5 shrink-0 ${COLORS[step.status]}`}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <span className="text-[12.5px] font-medium text-zinc-300">
                          {step.label}
                        </span>
                        <Badge variant="default">{step.status}</Badge>
                        <span className="font-mono text-[10.5px] text-zinc-500">
                          {step.duration_ms} ms
                        </span>
                      </div>
                      <p className="mt-0.5 break-words text-[11.5px] text-zinc-500">
                        {step.detail}
                      </p>
                    </div>
                  </div>
                );
              })}
            </div>
          </>
        ) : (
          <p className="rounded-lg border border-dashed border-zinc-800 px-3 py-4 text-center text-[12.5px] text-zinc-500">
            Nenhum teste executado ainda.
          </p>
        )}
      </CardContent>
    </Card>
  );
}
