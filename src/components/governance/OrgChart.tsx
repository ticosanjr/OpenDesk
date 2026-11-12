import { HeartPulse, Network, Pencil } from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { Tooltip } from "@/components/ui/tooltip";
import {
  budgetPercent,
  formatUsd,
  orgChildren,
  orgRoots,
  profileFor,
} from "@/lib/governance";
import { links } from "@/lib/links";
import type {
  AgentPerformance,
  AgentProfileRow,
  GovernanceAgent,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import { SectionTitle } from "./SectionTitle";

type TreeRow = { agent: GovernanceAgent; depth: number };

export function OrgChart({
  agents,
  profiles,
  performance,
  busy,
  onEdit,
  onHeartbeat,
}: {
  agents: GovernanceAgent[];
  profiles: AgentProfileRow[];
  performance: AgentPerformance[];
  busy: string | null;
  onEdit: (agent: GovernanceAgent) => void;
  onHeartbeat: (agent: GovernanceAgent) => void;
}) {
  const rows = flattenTree(agents, profiles);
  return (
    <section className="space-y-2">
      <SectionTitle
        icon={<Network size={13} />}
        title="Agentes"
        hint="Hierarquia, budget e uso nos últimos 30 dias"
      />
      {agents.length === 0 ? (
        <EmptyState
          icon={Network}
          title="Nenhum agente neste workspace"
          description="Crie agentes no Studio para montar o organograma."
          action={
            <Link
              to="/studio"
              className="text-[12px] text-blue-300 hover:text-blue-200"
            >
              Abrir Studio
            </Link>
          }
        />
      ) : (
        <Card className="overflow-hidden">
          <div className="scroll-area overflow-x-auto">
            <table className="w-full min-w-[760px] text-left">
              <thead className="border-b border-zinc-800/70 bg-zinc-950/40 text-[11px] uppercase tracking-[0.06em] text-zinc-400">
                <tr>
                  <th className="px-3 py-2.5 font-medium">Agente</th>
                  <th className="px-3 py-2.5 font-medium">Escopo</th>
                  <th className="px-3 py-2.5 text-right font-medium">Execuções 30d</th>
                  <th className="px-3 py-2.5 text-right font-medium">Sucesso</th>
                  <th className="px-3 py-2.5 font-medium">Budget</th>
                  <th className="px-3 py-2.5 text-right font-medium">Ações</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-zinc-800/50">
                {rows.map(({ agent, depth }) => (
                  <OrgRow
                    key={agent.id}
                    agent={agent}
                    depth={depth}
                    agents={agents}
                    profiles={profiles}
                    performance={performance}
                    busy={busy}
                    onEdit={onEdit}
                    onHeartbeat={onHeartbeat}
                  />
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      )}
    </section>
  );
}

function OrgRow({
  agent,
  depth,
  agents,
  profiles,
  performance,
  busy,
  onEdit,
  onHeartbeat,
}: {
  agent: GovernanceAgent;
  depth: number;
  agents: GovernanceAgent[];
  profiles: AgentProfileRow[];
  performance: AgentPerformance[];
  busy: string | null;
  onEdit: (agent: GovernanceAgent) => void;
  onHeartbeat: (agent: GovernanceAgent) => void;
}) {
  const row = profileFor(profiles, agent.id);
  const profile = row?.profile;
  const percent = row ? budgetPercent(row) : null;
  const perf = performance.find((item) => item.agent_id === agent.id);
  const childCount = orgChildren(agents, profiles, agent.id).length;
  const color = agent.color || "#60a5fa";
  const initials = Array.from(agent.avatar || agent.name || agent.id)
    .slice(0, 2)
    .join("");

  return (
    <tr
      data-agent-row
      data-agent-id={agent.id}
      className="align-top transition-colors hover:bg-zinc-900/30"
    >
      <td className="max-w-[420px] px-3 py-3">
        <div
          className="flex items-start gap-2.5"
          style={{ paddingLeft: depth * 18 }}
        >
          {depth > 0 ? (
            <span
              aria-hidden="true"
              className="mt-4 h-px w-3 shrink-0 bg-zinc-700"
            />
          ) : null}
          <span
            className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-zinc-700/60 text-[13px]"
            style={{ background: `${color}22`, color }}
          >
            {initials}
          </span>
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-1.5">
              <p className="truncate text-[14px] font-semibold text-zinc-100">
                {agent.name || agent.id}
              </p>
              <span className="font-mono text-[11px] text-zinc-500">
                @{agent.id}
              </span>
              {profile?.role ? (
                <Badge variant="purple">{profile.role}</Badge>
              ) : null}
              {profile?.team ? <Badge variant="default">{profile.team}</Badge> : null}
              {profile?.heartbeat_enabled ? (
                <Badge variant="green">
                  <HeartPulse size={10} />
                  heartbeat {profile.heartbeat_minutes}m
                </Badge>
              ) : null}
              {agent.disabled ? <Badge variant="red">desabilitado</Badge> : null}
            </div>
            <p className="mt-0.5 line-clamp-1 text-[12px] text-zinc-400">
              {agent.description || "Sem descrição"}
            </p>
            {childCount > 0 || profile?.manager_id ? (
              <p className="mt-0.5 text-[11px] text-zinc-500">
                {profile?.manager_id ? `reporta a @${profile.manager_id}` : null}
                {profile?.manager_id && childCount > 0 ? " · " : null}
                {childCount > 0
                  ? `${childCount} ${childCount === 1 ? "subordinado" : "subordinados"}`
                  : null}
              </p>
            ) : null}
          </div>
        </div>
      </td>
      <td className="px-3 py-3">
        <Badge variant={agent.scope === "workspace" ? "blue" : "default"}>
          {agent.scope}
        </Badge>
        {profile ? (
          <p className="mt-1.5 font-mono text-[11px] text-zinc-500">
            quota {profile.concurrency_quota}
          </p>
        ) : null}
      </td>
      <td className="px-3 py-3 text-right font-mono text-[13px] tabular-nums text-zinc-300">
        {perf ? perf.runs : "—"}
      </td>
      <td className="px-3 py-3 text-right">
        {perf ? (
          <span
            className={cn(
              "font-mono text-[13px] tabular-nums",
              perf.success_rate >= 0.9
                ? "text-emerald-300"
                : perf.success_rate >= 0.7
                  ? "text-amber-300"
                  : "text-red-300",
            )}
          >
            {Math.round(perf.success_rate * 100)}%
          </span>
        ) : (
          <span className="font-mono text-[12px] text-zinc-500">—</span>
        )}
      </td>
      <td className="min-w-[150px] px-3 py-3">
        {row ? (
          <>
            <p className="font-mono text-[12px] tabular-nums text-zinc-300">
              {formatUsd(row.monthly_spend_usd)} /{" "}
              {formatUsd(row.profile.monthly_budget_usd ?? null)}
            </p>
            {percent !== null ? (
              <div className="mt-1.5 h-1 max-w-[130px] overflow-hidden rounded-full bg-zinc-800">
                <div
                  className={cn(
                    "h-full rounded-full",
                    percent >= 90
                      ? "bg-red-400"
                      : percent >= 70
                        ? "bg-amber-400"
                        : "bg-emerald-400",
                  )}
                  style={{ width: `${Math.min(percent, 100)}%` }}
                />
              </div>
            ) : null}
          </>
        ) : (
          <p className="text-[12px] text-zinc-500">sem budget definido</p>
        )}
      </td>
      <td className="px-3 py-3">
        <div className="flex items-center justify-end gap-1.5">
          {profile ? (
            <Tooltip content="Criar uma tarefa de revisão de metas agora">
              <Button
                size="xs"
                variant="ghost"
                disabled={busy === `heartbeat-${agent.id}`}
                onClick={() => onHeartbeat(agent)}
              >
                <HeartPulse size={13} />
                <span className="hidden lg:inline">Check-in</span>
              </Button>
            </Tooltip>
          ) : null}
          <Button size="xs" variant="outline" onClick={() => onEdit(agent)}>
            <Pencil size={13} />
            Editar
          </Button>
          <Link
            to={links.agent(
              agent.id,
              agent.scope,
              agent.scope === "workspace" ? profile?.workspace_id : undefined,
            )}
            className="inline-flex h-7 items-center gap-1 rounded-md px-2 text-[12px] text-zinc-400 transition-colors hover:bg-zinc-800/60 hover:text-zinc-100"
            title="Abrir no Studio"
          >
            Studio
          </Link>
        </div>
      </td>
    </tr>
  );
}

/** Depth-first order so the table reads as the org chart it replaces. */
function flattenTree(
  agents: GovernanceAgent[],
  profiles: AgentProfileRow[],
): TreeRow[] {
  const seen = new Set<string>();
  const rows: TreeRow[] = [];
  const walk = (agent: GovernanceAgent, depth: number) => {
    if (seen.has(agent.id) || depth > 8) return;
    seen.add(agent.id);
    rows.push({ agent, depth });
    for (const child of orgChildren(agents, profiles, agent.id)) {
      walk(child, depth + 1);
    }
  };
  const roots = orgRoots(agents, profiles);
  for (const agent of roots.length > 0 ? roots : agents) walk(agent, 0);
  // Profiles can point at managers that are not in the current list.
  for (const agent of agents) walk(agent, 0);
  return rows;
}
