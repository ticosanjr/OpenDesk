import { Loader2, Play } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { IconLabel } from "@/components/ui/icon-label";
import { Input } from "@/components/ui/field";
import { ModelTestButton } from "@/components/ui/model-test-button";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import type { RouterTestResult } from "./router-types";

export function RouterPolicyTestCard({
  policy,
  workspaceId,
  title,
  description,
  tags,
  results,
  testingPolicy,
  onTitleChange,
  onDescriptionChange,
  onTagsChange,
  onTest,
  onTestAll,
}: {
  policy: string;
  workspaceId: string;
  title: string;
  description: string;
  tags: string;
  results: Record<string, RouterTestResult | undefined>;
  testingPolicy: string | null;
  onTitleChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onTagsChange: (value: string) => void;
  onTest: (policyId: string) => void;
  onTestAll: () => void;
}) {
  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Play size={15} className="text-blue-300" />
            Teste das modalidades
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Cada teste roda o classificador local do dispatch. Não cria tarefa,
            não abre sessão e não consome tokens.
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={onTestAll}
          disabled={testingPolicy !== null}
        >
          <IconLabel
            icon={
              testingPolicy ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Play size={13} />
              )
            }
          >
            {testingPolicy ? "Testando…" : "Testar todas"}
          </IconLabel>
        </Button>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="grid gap-2 md:grid-cols-[1fr_1.4fr_180px]">
          <Input
            value={title}
            onChange={(event) => onTitleChange(event.target.value)}
            placeholder="Título da tarefa de teste"
            aria-label="Título da tarefa de teste"
          />
          <Input
            value={description}
            onChange={(event) => onDescriptionChange(event.target.value)}
            placeholder="Descrição da tarefa de teste"
            aria-label="Descrição da tarefa de teste"
          />
          <Input
            value={tags}
            onChange={(event) => onTagsChange(event.target.value)}
            placeholder="tags, separadas"
            aria-label="Tags da tarefa de teste"
          />
        </div>
        <div className="scroll-area overflow-x-auto rounded-lg border border-zinc-800/70">
          <table className="w-full min-w-[760px] text-left text-[12px]">
            <thead className="bg-zinc-950/90 text-[11.5px] uppercase tracking-wide text-zinc-500">
              <tr>
                <th className="px-3 py-2">Modalidade</th>
                <th className="px-3 py-2">Decisão</th>
                <th className="px-3 py-2">Modelo resolvido</th>
                <th className="px-3 py-2 text-right">Testar</th>
              </tr>
            </thead>
            <tbody>
              {Object.entries(ROUTER_POLICY_LABELS).map(([id, label]) => {
                const result = results[id];
                const testing = testingPolicy === id;
                const active = id === policy;
                return (
                  <tr key={id} className="border-t border-zinc-800/60 align-top">
                    <td className="px-3 py-2.5">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <span className="font-medium text-zinc-200">{label}</span>
                        {active ? <Badge variant="blue">atual</Badge> : null}
                      </div>
                      <span className="font-mono text-[11.5px] text-zinc-500">
                        {id}
                      </span>
                    </td>
                    <td className="px-3 py-2.5 text-zinc-400">
                      {result ? (
                        <div className="space-y-1">
                          <div className="flex flex-wrap items-center gap-1.5">
                            <Badge variant="blue">{result.decision.origin}</Badge>
                            <span className="font-mono text-blue-100">
                              @{result.decision.selected_agent} ·{" "}
                              {result.decision.effort} · {result.decision.phase} ·{" "}
                              {result.decision.harness}
                            </span>
                          </div>
                          {result.decision.rule_name ? (
                            <p className="text-[11.5px] text-purple-300">
                              regra: {result.decision.rule_name}
                            </p>
                          ) : null}
                          <p
                            className="max-w-[340px] text-[11.5px] leading-snug text-zinc-500"
                            title={result.decision.rationale}
                          >
                            {result.decision.rationale}
                          </p>
                        </div>
                      ) : (
                        <span className="text-zinc-600">—</span>
                      )}
                    </td>
                    <td className="px-3 py-2.5">
                      {result ? (
                        <div className="space-y-1.5">
                          <span className="block truncate font-mono text-zinc-200">
                            {result.decision.selected_model || "(sem modelo)"}
                          </span>
                          <div className="flex flex-wrap items-center gap-1.5">
                            <Badge
                              variant={
                                !result.engine_online
                                  ? "yellow"
                                  : result.model_available
                                    ? "green"
                                    : "yellow"
                              }
                            >
                              {!result.engine_online
                                ? "motor offline"
                                : result.model_available
                                  ? "modelo listado"
                                  : "modelo não listado"}
                            </Badge>
                            <span className="text-[11.5px] text-zinc-500">
                              {result.available_models} modelo(s) no motor
                            </span>
                          </div>
                          <ModelTestButton
                            model={result.decision.selected_model}
                            workspaceId={workspaceId}
                            variant="compact"
                            label="Testar modelo"
                          />
                        </div>
                      ) : (
                        <span className="text-zinc-600">—</span>
                      )}
                    </td>
                    <td className="px-3 py-2.5 text-right">
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-6 px-2 text-[11.5px]"
                        onClick={() => onTest(id)}
                        disabled={testing}
                      >
                        {testing ? (
                          <Loader2 size={10} className="animate-spin" />
                        ) : (
                          <Play size={10} />
                        )}
                        Testar
                      </Button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        <p className="text-[12px] text-zinc-500">
          “Testar modelo” faz uma geração curta sem sessão, histórico ou
          ferramentas (<span className="font-mono">Output only: OK</span>) e
          consome poucos tokens. É assim que o OpenDesk separa quota do provider
          (402) de falha do motor.
        </p>
      </CardContent>
    </Card>
  );
}
