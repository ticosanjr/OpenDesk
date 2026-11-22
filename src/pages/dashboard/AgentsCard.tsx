import { Bot, TerminalSquare } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { AgentStateBadge } from "@/pages/dashboard/widgets";
import type { AgentFile, AgentInfo } from "@/lib/types";

export function AgentsCard({
  agents,
  presentation,
  agentStates,
  agentsError,
  onOpenStudio,
}: {
  agents: AgentInfo[];
  presentation: Record<string, AgentFile>;
  agentStates: Record<string, string>;
  agentsError: string | null;
  onOpenStudio: () => void;
}) {
  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Bot size={15} className="text-zinc-500" />
            Agentes ativos
          </CardTitle>
          <CardDescription>
            Lidos em tempo real de <span className="font-mono">/api/agent</span>{" "}
            com o diretório do workspace.
          </CardDescription>
        </div>
        <Badge variant="blue">{agents.length} encontrados</Badge>
      </CardHeader>
      <CardContent>
        {agentsError ? (
          <EmptyState
            icon={TerminalSquare}
            title="Não foi possível listar agentes"
            description={agentsError}
            compact
          />
        ) : agents.length === 0 ? (
          <EmptyState
            icon={Bot}
            title="Nenhum agente configurado"
            description="Crie um agente no OpenCode Studio ou adicione um arquivo em .opencode/agents/."
            compact
            action={
              <Button size="sm" variant="outline" onClick={onOpenStudio}>
                Abrir Studio
              </Button>
            }
          />
        ) : (
          <div className="grid gap-2 md:grid-cols-2">
            {agents.slice(0, 6).map((agent) => {
              const custom = presentation[agent.id];
              const displayName = custom?.name || agent.name || agent.id;
              const color = custom?.color || agent.color || "#60a5fa";
              const avatar =
                custom?.avatar ||
                Array.from(displayName).slice(0, 2).join("") ||
                "🤖";
              return (
                <div
                  key={agent.id}
                  className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3"
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-2.5">
                      <span
                        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border text-[13px]"
                        style={{
                          background: `${color}22`,
                          borderColor: `${color}44`,
                          color,
                        }}
                      >
                        {Array.from(avatar).slice(0, 2).join("")}
                      </span>
                      <div className="min-w-0">
                        <p className="truncate text-[13.5px] font-medium text-zinc-200">
                          {displayName}
                        </p>
                        <p className="truncate font-mono text-[11.5px] text-zinc-500">
                          @{agent.id}
                        </p>
                      </div>
                    </div>
                    <div className="flex flex-col items-end gap-1">
                      <Badge variant={agent.mode === "primary" ? "green" : "purple"}>
                        {agent.mode ?? "agent"}
                      </Badge>
                      <AgentStateBadge state={agentStates[agent.id]} />
                    </div>
                  </div>
                  <p className="mt-2 truncate text-[12px] text-zinc-500">
                    {custom?.description || agent.description || "sem descrição"}
                  </p>
                  <p className="mt-1 truncate font-mono text-[11.5px] text-zinc-500">
                    {agent.model
                      ? `${agent.model.providerID}/${agent.model.id}`
                      : "modelo herdado da sessão"}
                  </p>
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
