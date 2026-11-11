import {
  HeartPulse,
  Network,
  Pencil,
} from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
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
  const roots = orgRoots(agents, profiles);
  return (
    <section className="space-y-2">
      <SectionTitle
        icon={<Network size={13} />}
        title="Organograma"
        hint="Líderes e subordinados por workspace"
      />
      {agents.length === 0 ? (
        <EmptyState
          icon={Network}
          title="Nenhum agente neste workspace"
          description="Crie agentes no Studio para montar o organograma."
          action={
            <Link
              to="/studio"
              className="text-[11px] text-blue-300 hover:text-blue-200"
            >
              Abrir Studio
            </Link>
          }
        />
      ) : (
        <div className="space-y-2">
          {(roots.length > 0 ? roots : agents).map((agent) => (
            <OrgNode
              key={agent.id}
              agent={agent}
              agents={agents}
              profiles={profiles}
              performance={performance}
              depth={0}
              busy={busy}
              onEdit={onEdit}
              onHeartbeat={onHeartbeat}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function OrgNode({
  agent,
  agents,
  profiles,
  performance,
  depth,
  busy,
  onEdit,
  onHeartbeat,
}: {
  agent: GovernanceAgent;
  agents: GovernanceAgent[];
  profiles: AgentProfileRow[];
  performance: AgentPerformance[];
  depth: number;
  busy: string | null;
  onEdit: (agent: GovernanceAgent) => void;
  onHeartbeat: (agent: GovernanceAgent) => void;
}) {
  const row = profileFor(profiles, agent.id);
  const profile = row?.profile;
  const percent = row ? budgetPercent(row) : null;
  const perf = performance.find((item) => item.agent_id === agent.id);
  const children = orgChildren(agents, profiles, agent.id);
  const color = agent.color || "#60a5fa";
  const initials = Array.from(agent.avatar || agent.name || agent.id)
    .slice(0, 2)
    .join("");
  return (
    <div className={cn(depth > 0 && "ml-4 border-l border-zinc-800/80 pl-3")}>
      <Card className={cn(depth === 0 && "border-zinc-700/60")}>
        <CardContent className="p-3">
          <div className="flex flex-wrap items-start gap-3">
            <div
              className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-zinc-700/60 text-sm"
              style={{ background: `${color}22`, color }}
            >
              {initials}
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-1.5">
                <p className="text-[12px] font-medium text-zinc-100">
                  {agent.name || agent.id}
                </p>
                <span className="font-mono text-[9px] text-zinc-500">
                  @{agent.id}
                </span>
                <Badge variant={agent.scope === "workspace" ? "blue" : "default"}>
                  {agent.scope}
                </Badge>
                {profile?.role ? <Badge variant="purple">{profile.role}</Badge> : null}
                {profile?.team ? <Badge variant="default">{profile.team}</Badge> : null}
                {profile?.heartbeat_enabled ? (
                  <Badge variant="green">
                    <HeartPulse size={9} />
                    heartbeat {profile.heartbeat_minutes}m
                  </Badge>
                ) : null}
                {agent.disabled ? <Badge variant="red">desabilitado</Badge> : null}
              </div>
              <p className="mt-0.5 line-clamp-1 text-[9px] text-zinc-500">
                {agent.description || "Sem descrição"}
              </p>
              <div className="mt-1.5 flex flex-wrap items-center gap-3 text-[9px] text-zinc-500">
                <span>
                  {perf
                    ? `${perf.runs} runs · ${Math.round(perf.success_rate * 100)}% sucesso`
                    : "sem runs em 30d"}
                </span>
                {row ? (
                  <span>
                    budget {formatUsd(row.monthly_spend_usd)} /{" "}
                    {formatUsd(row.profile.monthly_budget_usd ?? null)}
                  </span>
                ) : (
                  <span>sem budget definido</span>
                )}
                {profile ? <span>quota {profile.concurrency_quota}</span> : null}
                {profile?.manager_id ? (
                  <span>reporta a @{profile.manager_id}</span>
                ) : null}
              </div>
              {percent !== null ? (
                <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-zinc-800">
                  <div
                    className={cn(
                      "h-full rounded-full",
                      percent >= 90
                        ? "bg-red-400"
                        : percent >= 70
                          ? "bg-amber-400"
                          : "bg-emerald-400",
                    )}
                    style={{ width: `${percent}%` }}
                  />
                </div>
              ) : null}
            </div>
            <div className="flex shrink-0 flex-wrap items-center gap-1.5">
              {profile ? (
                <Tooltip content="Criar uma tarefa de revisão de metas agora">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7"
                    disabled={busy === `heartbeat-${agent.id}`}
                    onClick={() => onHeartbeat(agent)}
                  >
                    <HeartPulse size={11} />
                    Check-in
                  </Button>
                </Tooltip>
              ) : null}
              <Button
                size="sm"
                variant="outline"
                className="h-7"
                onClick={() => onEdit(agent)}
              >
                <Pencil size={11} />
                Editar
              </Button>
              <Link
                to={links.agent(
                  agent.id,
                  agent.scope,
                  agent.scope === "workspace" ? profile?.workspace_id : undefined,
                )}
                className="inline-flex h-7 items-center gap-1 rounded-lg border border-zinc-800 px-2 text-[10px] text-zinc-300 hover:bg-zinc-900"
              >
                Studio
              </Link>
            </div>
          </div>
        </CardContent>
      </Card>
      {children.length > 0 && depth < 6 ? (
        <div className="mt-1.5 space-y-1.5">
          {children.map((child) => (
            <OrgNode
              key={child.id}
              agent={child}
              agents={agents}
              profiles={profiles}
              performance={performance}
              depth={depth + 1}
              busy={busy}
              onEdit={onEdit}
              onHeartbeat={onHeartbeat}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}
