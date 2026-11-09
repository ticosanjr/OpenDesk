import { useEffect, useState } from "react";
import { ArrowRight, CheckCircle2, ShieldCheck, XCircle } from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  delegationSummary,
  delegationTone,
} from "@/lib/governance";
import { relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import type { AgentDelegation, GovernanceAgent } from "@/lib/types";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import { SectionTitle } from "./SectionTitle";
import type { DelegationValues } from "./types";

export function DelegationsSection({
  agents,
  delegations,
  busy,
  onCreate,
  onResolve,
}: {
  agents: GovernanceAgent[];
  delegations: AgentDelegation[];
  busy: string | null;
  onCreate: (input: DelegationValues) => void;
  onResolve: (delegation: AgentDelegation, approved: boolean) => void;
}) {
  const [form, setForm] = useState<DelegationValues>({
    fromAgent: agents[0]?.id ?? "",
    toAgent: agents[1]?.id ?? agents[0]?.id ?? "",
    title: "",
    prompt: "",
  });
  useEffect(() => {
    setForm((current) => ({
      ...current,
      fromAgent: current.fromAgent || agents[0]?.id || "",
      toAgent:
        current.toAgent ||
        agents.find((agent) => agent.id !== (current.fromAgent || agents[0]?.id))?.id ||
        "",
    }));
  }, [agents]);
  const pending = delegations.filter((item) => item.status === "PENDING");
  return (
    <section className="space-y-2">
      <SectionTitle
        icon={<ShieldCheck size={13} />}
        title="Delegações"
        hint="agente → agente com aprovação humana"
      />
      <Card>
        <CardContent className="grid gap-2 p-3 md:grid-cols-2">
          <label>
            <span className={labelClass}>De</span>
            <select
              value={form.fromAgent}
              onChange={(event) =>
                setForm({ ...form, fromAgent: event.target.value })
              }
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
              value={form.toAgent}
              onChange={(event) => setForm({ ...form, toAgent: event.target.value })}
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
            <span className={labelClass}>Título</span>
            <input
              value={form.title}
              onChange={(event) => setForm({ ...form, title: event.target.value })}
              placeholder="Revisar o relatório trimestral"
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Prompt</span>
            <input
              value={form.prompt}
              onChange={(event) => setForm({ ...form, prompt: event.target.value })}
              placeholder="O que o agente deve fazer?"
              className={inputClass}
            />
          </label>
          <div className="md:col-span-2 flex justify-end">
            <Button
              size="sm"
              disabled={busy === "delegation" || agents.length < 2}
              onClick={() => onCreate(form)}
            >
              <ArrowRight size={12} />
              Solicitar delegação
            </Button>
          </div>
        </CardContent>
      </Card>
      {pending.length > 0 ? (
        <div className="space-y-1.5">
          {pending.map((delegation) => (
            <Card key={delegation.id} className="border-amber-500/25">
              <CardContent className="flex flex-wrap items-start gap-2 p-3">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <Badge variant="yellow">pendente</Badge>
                    <p className="text-[11px] font-medium text-zinc-200">
                      {delegation.title}
                    </p>
                    <span className="font-mono text-[9px] text-zinc-600">
                      {delegationSummary(delegation)}
                    </span>
                  </div>
                  <p className="mt-1 line-clamp-2 text-[10px] leading-relaxed text-zinc-500">
                    {delegation.prompt}
                  </p>
                  <p className="mt-1 text-[9px] text-zinc-600">
                    solicitada {relativeTime(delegation.requested_at)}
                  </p>
                </div>
                <div className="flex shrink-0 gap-1.5">
                  <Button
                    size="sm"
                    disabled={busy === `delegation-${delegation.id}`}
                    onClick={() => onResolve(delegation, true)}
                  >
                    <CheckCircle2 size={11} />
                    Aprovar e executar
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    className="border-red-500/30 text-red-300"
                    disabled={busy === `delegation-${delegation.id}`}
                    onClick={() => onResolve(delegation, false)}
                  >
                    <XCircle size={11} />
                    Rejeitar
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      ) : null}
      {delegations.length > pending.length ? (
        <div className="flex flex-wrap gap-1.5">
          {delegations
            .filter((item) => item.status !== "PENDING")
            .slice(0, 6)
            .map((delegation) => (
              <div
                key={delegation.id}
                className="flex items-center gap-1.5 rounded-lg border border-zinc-800 bg-zinc-950/30 px-2 py-1"
              >
                <Badge variant={delegationTone(delegation.status)}>
                  {delegation.status}
                </Badge>
                <span className="text-[9px] text-zinc-500">{delegation.title}</span>
                <span className="font-mono text-[9px] text-zinc-600">
                  {delegationSummary(delegation)}
                </span>
                {delegation.task_id ? (
                  <Link
                    to={links.task(delegation.task_id)}
                    className="text-[9px] text-blue-300 hover:text-blue-200"
                  >
                    tarefa
                  </Link>
                ) : null}
              </div>
            ))}
        </div>
      ) : null}
    </section>
  );
}
