import {
  CircleDollarSign,
  GitBranch,
  Gauge,
  Plus,
  Route,
  ShieldAlert,
  SlidersHorizontal,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import { formatUsd } from "@/lib/format";

const POLICIES = [
  {
    id: "COST_BENEFIT",
    title: "Melhor custo-benefício",
    description:
      "Flash para triagem; modelo forte para Plan e Build; effort resolvido por variante.",
  },
  {
    id: "CHEAPEST",
    title: "Mais barato",
    description:
      "Tudo no modelo mais econômico disponível, com effort low e sem escalonamento.",
  },
  {
    id: "BEST_QUALITY",
    title: "Máxima qualidade",
    description:
      "Plan → Build → Audit encadeados, com gate humano e auditor read-only.",
  },
  {
    id: "MANUAL",
    title: "Manual",
    description:
      "Você escolhe modo, subagente, modelo e effort no dropdown antes de executar.",
  },
  {
    id: "CUSTOM_RULES",
    title: "Regras customizadas",
    description:
      "Avaliação IF-THEN por prioridade sobre tags, complexidade, workspace e orçamento.",
  },
];

export function RouterPage() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const activePolicy = workspace?.default_router_policy ?? "COST_BENEFIT";

  return (
    <PageBody>
      <PageHeader
        eyebrow="governança"
        title="Roteador & Custos"
        description="Política global, regras condicionais e limites de gasto que bloqueiam a fila antes de estourar o orçamento."
        actions={<Badge variant="default">fase 3</Badge>}
      />

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
        {POLICIES.map((policy) => {
          const active = policy.id === activePolicy;
          return (
            <Card
              key={policy.id}
              className={active ? "border-blue-500/40 bg-blue-500/[0.04]" : undefined}
            >
              <CardContent className="p-4">
                <div className="flex items-center justify-between">
                  <SlidersHorizontal
                    size={15}
                    className={active ? "text-blue-400" : "text-zinc-600"}
                  />
                  {active ? <Badge variant="blue">atual</Badge> : null}
                </div>
                <p className="mt-3 text-[12px] font-medium text-zinc-200">
                  {policy.title}
                </p>
                <p className="mt-1 text-[10px] leading-relaxed text-zinc-600">
                  {policy.description}
                </p>
                <p className="mt-2 font-mono text-[9px] text-zinc-700">
                  {ROUTER_POLICY_LABELS[policy.id]}
                </p>
              </CardContent>
            </Card>
          );
        })}
      </div>

      <div className="grid gap-3 lg:grid-cols-3">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <CircleDollarSign size={15} className="text-zinc-500" />
              Budget Guard
            </CardTitle>
            <Badge variant="yellow">hard cap</Badge>
          </CardHeader>
          <CardContent className="space-y-3">
            <BudgetRow label="Limite diário" value={formatUsd(5)} />
            <BudgetRow label="Limite mensal" value={formatUsd(50)} />
            <BudgetRow label="Alerta soft" value="80%" />
            <BudgetRow label="Janela opencode-go" value="5h · 5M tokens" />
            <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] leading-relaxed text-amber-300">
              Ao atingir o hard cap, novas execuções são pausadas e uma notificação
              nativa é disparada. Runs em andamento são interrompidos se excederem o
              teto por job.
            </p>
          </CardContent>
        </Card>

        <Card className="lg:col-span-2">
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <GitBranch size={15} className="text-zinc-500" />
                Regras IF-THEN
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                Avaliadas em ordem de prioridade; a primeira que casar vence.
              </p>
            </div>
            <Button size="sm" disabled>
              <Plus size={14} />
              Nova regra
            </Button>
          </CardHeader>
          <CardContent>
            <EmptyState
              icon={Route}
              title="Nenhuma regra customizada"
              description="Exemplos: tags contém 'roteiro' → build/@roteirista/effort high; budget diário > 80% → forçar flash/low; complexidade HIGH → Plan antes de Build."
            />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ShieldAlert size={15} className="text-zinc-500" />
            Effort no OpenCode V2
          </CardTitle>
          <Badge variant="default">variantes</Badge>
        </CardHeader>
        <CardContent className="flex items-start gap-3">
          <Gauge size={16} className="mt-0.5 text-zinc-600" />
          <p className="text-[11px] leading-relaxed text-zinc-500">
            O V2 não expõe um parâmetro <span className="font-mono">effort</span>{" "}
            isolado. O OpenDesk resolve low/medium/high para variantes do modelo
            quando <span className="font-mono">/api/model</span> as expõe; caso
            contrário, mantém o effort como metadado de roteamento e seleciona o
            modelo compatível com a política.
          </p>
        </CardContent>
      </Card>
    </PageBody>
  );
}

function BudgetRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 text-[11px]">
      <span className="text-zinc-600">{label}</span>
      <span className="font-mono text-[10px] text-zinc-300">{value}</span>
    </div>
  );
}
