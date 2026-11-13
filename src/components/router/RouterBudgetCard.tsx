import { CircleDollarSign, Loader2, Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { formatUsd } from "@/lib/format";
import type { BudgetSettings } from "@/lib/types";
import type { RouterConfig } from "./router-types";

function BudgetInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-[11.5px] uppercase tracking-[0.12em] text-zinc-500">
        {label}
      </span>
      <input
        type="number"
        min={0}
        step={label.includes("tokens") ? 1000 : 0.01}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
        className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 font-mono text-[13px] text-zinc-100 outline-none focus:border-blue-500/60"
      />
    </label>
  );
}

export function RouterBudgetCard({
  config,
  settings,
  saving,
  onSettingsChange,
  onSave,
}: {
  config: RouterConfig | null;
  settings: BudgetSettings | null;
  saving: boolean;
  onSettingsChange: (settings: BudgetSettings) => void;
  onSave: () => void;
}) {
  const update = (patch: Partial<BudgetSettings>) => {
    if (!settings) return;
    onSettingsChange({ ...settings, ...patch });
  };

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <CircleDollarSign size={15} className="text-zinc-500" />
            Budget Guard
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            {config?.budget.hard_stop ? "hard stop ativo" : "dentro do orçamento"}
          </p>
        </div>
        <Button size="sm" onClick={onSave} disabled={saving || !settings}>
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
          Salvar
        </Button>
      </CardHeader>
      <CardContent className="space-y-3">
        <BudgetInput
          label="Limite diário (USD)"
          value={settings?.daily_limit_usd ?? 5}
          onChange={(value) => update({ daily_limit_usd: value })}
        />
        <BudgetInput
          label="Limite mensal (USD)"
          value={settings?.monthly_limit_usd ?? 50}
          onChange={(value) => update({ monthly_limit_usd: value })}
        />
        <BudgetInput
          label="Alerta soft (%)"
          value={settings?.soft_alert_threshold_pct ?? 80}
          onChange={(value) => update({ soft_alert_threshold_pct: value })}
        />
        <BudgetInput
          label="Limite 5h opencode-go (tokens)"
          value={settings?.opencode_go_5h_token_limit ?? 5_000_000}
          onChange={(value) =>
            update({ opencode_go_5h_token_limit: Math.round(value) })
          }
        />
        <label className="flex items-center gap-2 text-[13px] text-zinc-400">
          <input
            type="checkbox"
            checked={settings?.hard_stop_enabled ?? true}
            onChange={(event) => update({ hard_stop_enabled: event.target.checked })}
            className="accent-blue-500"
          />
          hard stop habilitado
        </label>
        <div className="grid grid-cols-2 gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-2.5 text-[12px]">
          <span className="text-zinc-500">gasto hoje</span>
          <span className="text-right font-mono text-zinc-300">
            {formatUsd(config?.budget.daily_spent ?? 0)} /{" "}
            {formatUsd(config?.budget.daily_limit ?? 0)}
          </span>
          <span className="text-zinc-500">gasto no mês</span>
          <span className="text-right font-mono text-zinc-300">
            {formatUsd(config?.budget.monthly_spent ?? 0)} /{" "}
            {formatUsd(config?.budget.monthly_limit ?? 0)}
          </span>
          <span className="text-zinc-500">janela 5h</span>
          <span className="text-right font-mono text-zinc-300">
            {config?.budget.five_hour_tokens ?? 0} tokens
          </span>
        </div>
        {config?.budget.hard_stop ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] leading-relaxed text-red-300">
            Hard cap atingido: a fila é pausada e notificada. Ajuste o limite ou
            aguarde o próximo período.
          </p>
        ) : config?.budget.soft_alert ? (
          <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] leading-relaxed text-amber-300">
            Alerta soft: o orçamento passou de{" "}
            {settings?.soft_alert_threshold_pct ?? 80}%.
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}
