import { Gauge, Loader2, Route, Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { IconLabel } from "@/components/ui/icon-label";
import { LoadingState } from "@/components/ui/loading-state";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { cn } from "@/lib/utils";
import type { RouterAnalysis } from "./router-types";

export function RouterCostCard({
  routerModel,
  savingModel,
  costBlend,
  savingBlend,
  analysis,
  analyzing,
  onModelChange,
  onSaveModel,
  onBlendChange,
  onSaveBlend,
  onAnalyze,
}: {
  routerModel: string;
  savingModel: boolean;
  costBlend: number;
  savingBlend: boolean;
  analysis: RouterAnalysis | null;
  analyzing: boolean;
  onModelChange: (value: string) => void;
  onSaveModel: () => void;
  onBlendChange: (value: number) => void;
  onSaveBlend: () => void;
  onAnalyze: () => void;
}) {
  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Route size={15} className="text-zinc-500" />
            Modelo roteador e comparação
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Escolha o modelo usado como referência e execute a comparação de
            custo quando quiser.
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={onAnalyze} disabled={analyzing}>
          <IconLabel
            icon={
              <Gauge size={13} className={analyzing ? "animate-pulse" : undefined} />
            }
          >
            {analyzing ? "Comparando…" : "Executar comparação"}
          </IconLabel>
        </Button>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex flex-wrap items-end gap-2">
          <label className="min-w-[240px] flex-1">
            <span className="mb-1.5 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
              Modelo roteador
            </span>
            <ModelCombobox
              value={routerModel || null}
              onChange={(value) => onModelChange(value ?? "")}
              routerLabel="Determinístico (local)"
              routerHint="Sem modelo de IA: o roteador compara custo e qualidade com regras locais."
              ariaLabel="Modelo roteador"
            />
          </label>
          <Button size="sm" className="h-9" onClick={onSaveModel} disabled={savingModel}>
            <IconLabel
              icon={
                savingModel ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Save size={13} />
                )
              }
            >
              Salvar modelo
            </IconLabel>
          </Button>
        </div>
        <div className="flex flex-wrap items-end gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-2.5">
          <label className="min-w-[220px] flex-1">
            <span className="mb-1.5 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
              Peso da entrada no custo combinado: {Math.round(costBlend * 100)}%
              entrada / {Math.round((1 - costBlend) * 100)}% saída
            </span>
            <input
              aria-label="Peso da entrada"
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={costBlend}
              onChange={(event) => onBlendChange(Number(event.target.value))}
              className="w-full accent-blue-500"
            />
          </label>
          <Button
            size="sm"
            variant="outline"
            className="h-8"
            onClick={onSaveBlend}
            disabled={savingBlend}
          >
            <IconLabel icon={<Save size={12} />}>Salvar blend</IconLabel>
          </Button>
        </div>
        {analysis ? (
          <div className="space-y-2">
            <p className="rounded-lg border border-blue-500/20 bg-blue-500/[0.05] px-3 py-2 text-[13px] leading-snug text-blue-100">
              {analysis.recommendation}
            </p>
            <p className="text-[12px] text-zinc-500">
              {analysis.models.length} modelo(s) comparado(s). Mais barato:{" "}
              <span className="font-mono text-emerald-300">
                {analysis.cheapest ?? "—"}
              </span>
            </p>
            {analysis.models.length === 0 ? (
              <p className="rounded-lg border border-dashed border-zinc-700 px-3 py-4 text-center text-[12px] text-zinc-500">
                Nenhum modelo com preço disponível. Verifique se o OpenCode está
                online.
              </p>
            ) : (
              <div className="scroll-area max-h-72 rounded-lg border border-zinc-800/70">
                <table className="w-full text-left text-[12px]">
                  <thead className="sticky top-0 bg-zinc-950/95 text-[11.5px] uppercase tracking-wide text-zinc-500">
                    <tr>
                      <th className="px-2.5 py-1.5">Modelo</th>
                      <th className="px-2.5 py-1.5 text-right">Entrada /1M</th>
                      <th className="px-2.5 py-1.5 text-right">Saída /1M</th>
                      <th className="px-2.5 py-1.5 text-right">Combinado</th>
                    </tr>
                  </thead>
                  <tbody>
                    {analysis.models.map((row) => (
                      <tr
                        key={row.model}
                        className={cn(
                          "border-t border-zinc-800/60",
                          row.model === analysis.cheapest &&
                            "bg-emerald-500/[0.05]",
                        )}
                      >
                        <td className="px-2.5 py-1.5">
                          <span className="block truncate font-mono text-zinc-200">
                            {row.model}
                          </span>
                          <span className="block truncate text-[11.5px] text-zinc-500">
                            {row.name}
                          </span>
                        </td>
                        <td className="px-2.5 py-1.5 text-right font-mono text-zinc-400">
                          ${row.input_cost.toFixed(3)}
                        </td>
                        <td className="px-2.5 py-1.5 text-right font-mono text-zinc-400">
                          ${row.output_cost.toFixed(3)}
                        </td>
                        <td className="px-2.5 py-1.5 text-right font-mono text-zinc-200">
                          ${row.blended_cost.toFixed(3)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        ) : analyzing ? (
          <LoadingState compact rows={3} label="Comparando custos dos modelos…" />
        ) : null}
      </CardContent>
    </Card>
  );
}
