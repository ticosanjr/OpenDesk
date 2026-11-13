import { Gauge, Loader2, Save } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { IconLabel } from "@/components/ui/icon-label";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { fieldLabelClass as labelClass } from "@/lib/ui-classes";

export function RouterModelsCard({
  defaultModel,
  manualModel,
  saving,
  onDefaultModelChange,
  onManualModelChange,
  onSave,
}: {
  defaultModel: string;
  manualModel: string;
  saving: boolean;
  onDefaultModelChange: (value: string) => void;
  onManualModelChange: (value: string) => void;
  onSave: () => void;
}) {
  return (
    <Card className="border-zinc-800/80 bg-zinc-950/40">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Gauge size={15} className="text-blue-300" />
            Modelos do roteador
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Modelos escolhidos no OpenDesk para este workspace. O roteador não
            lê mais o <span className="font-mono">model</span> do{" "}
            <span className="font-mono">opencode.jsonc</span>.
          </p>
        </div>
        <Button size="sm" onClick={onSave} disabled={saving}>
          <IconLabel
            icon={
              saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />
            }
          >
            Salvar modelos
          </IconLabel>
        </Button>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="grid gap-5 md:grid-cols-2">
          <div>
            <span className={labelClass}>Modelo padrão do OpenDesk</span>
            <ModelCombobox
              value={defaultModel || null}
              onChange={(value) => onDefaultModelChange(value ?? "")}
              allowRouter={false}
              placeholder="deepseek/deepseek-flash"
              ariaLabel="Modelo padrão"
            />
            <p className="mt-1.5 text-[12px] leading-snug text-zinc-500">
              Base das políticas automáticas e fallback do roteador. Se o modelo
              não existir no motor, o roteador escolhe outro modelo disponível e
              registra a origem <span className="font-mono">FALLBACK</span>.
            </p>
          </div>
          <div>
            <span className={labelClass}>Modelo do modo Manual</span>
            <ModelCombobox
              value={manualModel || null}
              onChange={(value) => onManualModelChange(value ?? "")}
              allowRouter={false}
              placeholder="escolha o modelo usado no manual"
              ariaLabel="Modelo do modo manual"
            />
            <p className="mt-1.5 text-[12px] leading-snug text-zinc-500">
              Usado quando a política é <strong>Manual</strong> e a tarefa não
              escolhe um modelo. O modelo definido no card da tarefa continua
              vencendo.
            </p>
          </div>
        </div>
        {!defaultModel ? (
          <p className="rounded-lg border border-amber-500/20 bg-amber-500/[0.06] px-3 py-2 text-[12px] leading-snug text-amber-200">
            Nenhum modelo padrão definido. Escolha um acima para evitar que o
            roteador dependa do modelo padrão do OpenCode ou de um provider que
            você não usa.
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}
