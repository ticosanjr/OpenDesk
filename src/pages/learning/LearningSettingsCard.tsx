import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import type { LearningSettings } from "@/lib/types";
import { smallInputClass, smallLabelClass } from "@/lib/ui-classes";
import { cn } from "@/lib/utils";
import { SettingToggle } from "@/pages/learning/SettingToggle";

export function LearningSettingsCard({
  settings,
  savingSettings,
  onSave,
}: {
  settings: LearningSettings;
  savingSettings: boolean;
  onSave: (next: LearningSettings) => void;
}) {
  return (
    <Card>
      <CardContent className="space-y-3 p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <p className="text-[13.5px] font-medium text-zinc-200">
              Captura leve e controlada
            </p>
            <p className="mt-0.5 max-w-2xl text-[12px] leading-relaxed text-zinc-500">
              O OpenDesk observa o resultado das execuções e agrupa padrões
              repetidos. Sem embeddings, sem pipeline pesado e sem prompts
              completos.
            </p>
          </div>
          {savingSettings ? (
            <Badge variant="default">salvando…</Badge>
          ) : null}
        </div>
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <SettingToggle
            label="Ativar aprendizado"
            description="Observa execuções concluídas."
            checked={settings.enabled}
            onCheckedChange={(enabled) => onSave({ ...settings, enabled })}
          />
          <SettingToggle
            label="Capturar sucessos"
            description="Também aprende com execuções bem-sucedidas."
            checked={settings.capture_successes}
            disabled={!settings.enabled}
            onCheckedChange={(capture_successes) =>
              onSave({ ...settings, capture_successes })
            }
          />
          <SettingToggle
            label="Propor automaticamente"
            description="Cria proposta ao atingir o mínimo."
            checked={settings.auto_propose}
            disabled={!settings.enabled}
            onCheckedChange={(auto_propose) =>
              onSave({ ...settings, auto_propose })
            }
          />
          <label>
            <span className={smallLabelClass}>Mínimo de ocorrências</span>
            <input
              type="number"
              min={1}
              max={20}
              value={settings.min_occurrences}
              disabled={!settings.enabled || savingSettings}
              onChange={(event) =>
                onSave({
                  ...settings,
                  min_occurrences: Number(event.target.value) || 1,
                })
              }
              className={cn(smallInputClass, "font-mono")}
              aria-label="Mínimo de ocorrências"
            />
          </label>
        </div>
      </CardContent>
    </Card>
  );
}
