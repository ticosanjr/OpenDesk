import { useCallback, useEffect, useState } from "react";
import { ArrowRight, CheckCircle2, Package, Plus, XCircle } from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiGet, apiPost } from "@/lib/api";
import { relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import type { GovernanceAgent } from "@/lib/types";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { SectionTitle } from "./SectionTitle";

interface Handoff {
  id: string;
  task_id?: string | null;
  parent_handoff_id?: string | null;
  source_agent: string;
  target_agent: string;
  objective: string;
  input_artifacts_json: string;
  acceptance_json: string;
  sub_budget_usd?: number | null;
  status: string;
  result_json?: string | null;
  created_at: string;
}

function statusTone(status: string): "green" | "red" | "blue" | "yellow" | "default" {
  switch (status) {
    case "DONE":
      return "green";
    case "FAILED":
    case "REJECTED":
      return "red";
    case "RUNNING":
    case "ACCEPTED":
      return "blue";
    case "PENDING":
      return "yellow";
    default:
      return "default";
  }
}

function resultTaskId(handoff: Handoff): string | null {
  if (!handoff.result_json) return null;
  try {
    const value = JSON.parse(handoff.result_json) as { task_id?: string };
    return value.task_id ?? null;
  } catch {
    return null;
  }
}

export function HandoffsSection({
  workspaceId,
  agents,
}: {
  workspaceId: string;
  agents: GovernanceAgent[];
}) {
  const [handoffs, setHandoffs] = useState<Handoff[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [form, setForm] = useState({
    from: "",
    to: "",
    objective: "",
    artifacts: "",
    acceptance: "",
    budget: "",
  });

  useEffect(() => {
    setForm((current) => ({
      ...current,
      from: current.from || agents[0]?.id || "",
      to:
        current.to ||
        agents.find((agent) => agent.id !== (current.from || agents[0]?.id))?.id ||
        "",
    }));
  }, [agents]);

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      setHandoffs(
        await apiGet<Handoff[]>(
          `/api/handoffs?workspace_id=${encodeURIComponent(workspaceId)}&limit=50`,
        ),
      );
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar os handoffs",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function create() {
    if (!form.objective.trim()) {
      toast({ tone: "error", title: "Descreva o objetivo do handoff" });
      return;
    }
    setBusy("create");
    try {
      await apiPost(`/api/handoffs?workspace_id=${encodeURIComponent(workspaceId)}`, {
        source_agent: form.from,
        target_agent: form.to,
        objective: form.objective.trim(),
        input_artifacts: form.artifacts
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean),
        acceptance: form.acceptance
          .split("\n")
          .map((value) => value.trim())
          .filter(Boolean),
        sub_budget_usd: form.budget.trim() ? Number(form.budget) : null,
      });
      setForm({ ...form, objective: "", artifacts: "", acceptance: "", budget: "" });
      await load();
      toast({
        tone: "success",
        title: "Handoff criado",
        description: "Ele precisa de aceite humano antes de gerar tarefa.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao criar o handoff",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function respond(handoff: Handoff, decision: "accept" | "reject") {
    const approved = await confirmAction({
      title:
        decision === "accept"
          ? `Aceitar handoff para @${handoff.target_agent}?`
          : "Rejeitar este handoff?",
      description:
        decision === "accept"
          ? `Aceitar cria uma tarefa para @${handoff.target_agent}${
              handoff.sub_budget_usd
                ? ` com sub-orçamento de US$ ${handoff.sub_budget_usd.toFixed(2)}`
                : ""
            }.`
          : "O agente de origem verá a recusa; nenhuma tarefa é criada.",
      confirmLabel: decision === "accept" ? "Aceitar" : "Rejeitar",
      tone: decision === "accept" ? "default" : "destructive",
    });
    if (!approved) return;
    setBusy(handoff.id);
    try {
      await apiPost(
        `/api/handoffs/${handoff.id}/${decision === "accept" ? "accept" : "reject"}`,
        decision === "accept" ? {} : { reason: "recusado na governança" },
      );
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      if (decision === "accept" && message.includes("orçamento")) {
        const force = await confirmAction({
          title: "Aceitar acima do orçamento?",
          description: message,
          confirmLabel: "Aceitar mesmo assim",
          tone: "destructive",
        });
        if (!force) return;
        try {
          await apiPost(`/api/handoffs/${handoff.id}/accept`, { force: true });
        } catch (forced) {
          toast({
            tone: "error",
            title: "Aceite forçado falhou",
            description: forced instanceof Error ? forced.message : String(forced),
          });
          return;
        }
      } else {
        toast({
          tone: "error",
          title: "Não foi possível responder",
          description: message,
        });
        return;
      }
    } finally {
      setBusy(null);
    }
    await load();
  }

  async function finish(handoff: Handoff, status: "DONE" | "FAILED") {
    setBusy(handoff.id);
    try {
      await apiPost(`/api/handoffs/${handoff.id}/finish`, { status });
      await load();
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="space-y-2">
      <SectionTitle
        icon={<Package size={13} />}
        title="Handoffs de tarefa"
        hint="objetivo, artefatos, aceite e sub-orçamento"
      />
      <Card>
        <CardContent className="grid gap-2 p-3 md:grid-cols-2">
          <label>
            <span className={labelClass}>De</span>
            <select
              value={form.from}
              onChange={(event) => setForm({ ...form, from: event.target.value })}
              className={inputClass}
            >
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  @{agent.id}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className={labelClass}>Para</span>
            <select
              value={form.to}
              onChange={(event) => setForm({ ...form, to: event.target.value })}
              className={inputClass}
            >
              {agents
                .filter((agent) => agent.id !== form.from)
                .map((agent) => (
                  <option key={agent.id} value={agent.id}>
                    @{agent.id}
                  </option>
                ))}
            </select>
          </label>
          <label className="md:col-span-2">
            <span className={labelClass}>Objetivo</span>
            <textarea
              value={form.objective}
              onChange={(event) => setForm({ ...form, objective: event.target.value })}
              rows={2}
              placeholder="O que o agente destino deve entregar?"
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Artefatos de entrada (vírgula)</span>
            <input
              value={form.artifacts}
              onChange={(event) => setForm({ ...form, artifacts: event.target.value })}
              placeholder="output/2026-11/relatorio/final.md"
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Sub-orçamento (US$, opcional)</span>
            <input
              value={form.budget}
              onChange={(event) => setForm({ ...form, budget: event.target.value })}
              inputMode="decimal"
              placeholder="2.50"
              className={inputClass}
            />
          </label>
          <label className="md:col-span-2">
            <span className={labelClass}>Critérios de aceite (um por linha)</span>
            <textarea
              value={form.acceptance}
              onChange={(event) =>
                setForm({ ...form, acceptance: event.target.value })
              }
              rows={2}
              placeholder="Sem erros factuais&#10;Citar as fontes usadas"
              className={inputClass}
            />
          </label>
          <div className="md:col-span-2 flex justify-end">
            <Button
              size="sm"
              className="h-7"
              disabled={busy === "create"}
              onClick={() => void create()}
            >
              <Plus size={12} />
              Criar handoff
            </Button>
          </div>
        </CardContent>
      </Card>

      {loading ? (
        <p className="text-[12.5px] text-zinc-500">carregando handoffs…</p>
      ) : handoffs.length === 0 ? (
        <EmptyState
          title="Nenhum handoff"
          description="Agentes podem delegar trabalho com contrato explícito; tudo passa por aceite humano."
        />
      ) : (
        <div className="space-y-2">
          {handoffs.map((handoff) => {
            const taskId = resultTaskId(handoff) ?? handoff.task_id ?? null;
            const acceptance = (() => {
              try {
                return JSON.parse(handoff.acceptance_json) as string[];
              } catch {
                return [];
              }
            })();
            return (
              <Card key={handoff.id}>
                <CardContent className="flex flex-wrap items-start justify-between gap-3 p-3">
                  <div className="min-w-0 space-y-1">
                    <div className="flex flex-wrap items-center gap-1.5">
                      <Badge variant={statusTone(handoff.status)}>
                        {handoff.status}
                      </Badge>
                      <span className="flex items-center gap-1 text-[12.5px] font-medium text-zinc-200">
                        @{handoff.source_agent}
                        <ArrowRight size={11} className="text-zinc-500" />
                        @{handoff.target_agent}
                      </span>
                      {handoff.sub_budget_usd ? (
                        <span className="text-[11.5px] text-zinc-500">
                          US$ {handoff.sub_budget_usd.toFixed(2)}
                        </span>
                      ) : null}
                    </div>
                    <p className="text-[13px] text-zinc-300">{handoff.objective}</p>
                    {acceptance.length > 0 ? (
                      <p className="text-[11.5px] text-zinc-500">
                        {acceptance.length} critério(s) de aceite
                      </p>
                    ) : null}
                    <p className="text-[11px] text-zinc-600">
                      {relativeTime(handoff.created_at)}
                      {taskId ? (
                        <>
                          {" · "}
                          <Link
                            to={links.task(taskId)}
                            className="text-blue-300 hover:underline"
                          >
                            tarefa gerada
                          </Link>
                        </>
                      ) : null}
                    </p>
                  </div>
                  <div className="flex items-center gap-1.5">
                    {handoff.status === "PENDING" ? (
                      <>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7"
                          disabled={busy === handoff.id}
                          onClick={() => void respond(handoff, "accept")}
                        >
                          <CheckCircle2 size={11} />
                          Aceitar
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7"
                          disabled={busy === handoff.id}
                          onClick={() => void respond(handoff, "reject")}
                        >
                          <XCircle size={11} />
                          Rejeitar
                        </Button>
                      </>
                    ) : null}
                    {handoff.status === "ACCEPTED" || handoff.status === "RUNNING" ? (
                      <>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7"
                          disabled={busy === handoff.id}
                          onClick={() => void finish(handoff, "DONE")}
                        >
                          Concluir
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7"
                          disabled={busy === handoff.id}
                          onClick={() => void finish(handoff, "FAILED")}
                        >
                          Falhou
                        </Button>
                      </>
                    ) : null}
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
    </section>
  );
}
