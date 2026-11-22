import { LabeledField, inputClass } from "../recipe-fields";
import { CHANNELS, HITL_TYPES, ON_TIMEOUT } from "./recipe-editor-model";
import type { Dictionary } from "../recipe-types";

export function HitlEditor({
  checkpoint,
  onChange,
}: {
  checkpoint: Dictionary;
  onChange: (value: Dictionary) => void;
}) {
  const type = String(checkpoint.interaction_type ?? "BINARY_GATE");
  const channels = Array.isArray(checkpoint.channels)
    ? (checkpoint.channels as string[])
    : [];
  return (
    <div className="mt-1.5 space-y-1.5">
      <div className="grid gap-1.5 sm:grid-cols-2">
        <LabeledField
          label="Tipo de interação"
          hint="Decide como o usuário responde ao checkpoint."
        >
          <select
            value={type}
            onChange={(event) => onChange({ interaction_type: event.target.value })}
            className={inputClass}
          >
            {HITL_TYPES.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </LabeledField>
        <LabeledField
          label="Tempo limite (min)"
          hint="Depois disso, a política de timeout é aplicada."
        >
          <input
            type="number"
            min={1}
            value={Number(checkpoint.timeout_minutes ?? 240)}
            onChange={(event) =>
              onChange({ timeout_minutes: Number(event.target.value) })
            }
            className={inputClass}
          />
        </LabeledField>
        <LabeledField label="Se o tempo acabar">
          <select
            value={String(checkpoint.on_timeout ?? "PAUSE_AND_ALERT")}
            onChange={(event) => onChange({ on_timeout: event.target.value })}
            className={inputClass}
          >
            {ON_TIMEOUT.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </LabeledField>
        <LabeledField label="Canais de aviso">
          <div className="flex flex-wrap gap-2 pt-1.5">
            {CHANNELS.map((channel) => (
              <label key={channel} className="flex items-center gap-1 text-[12px] text-zinc-400">
                <input
                  type="checkbox"
                  checked={channels.includes(channel)}
                  onChange={(event) =>
                    onChange({
                      channels: event.target.checked
                        ? [...channels, channel]
                        : channels.filter((item) => item !== channel),
                    })
                  }
                  className="accent-blue-500"
                />
                {channel}
              </label>
            ))}
          </div>
        </LabeledField>
      </div>
      {type === "MULTI_CHOICE" ? (
        <LabeledField
          label="Opções (separadas por vírgula)"
          hint="O usuário escolhe uma destas opções."
        >
          <input
            value={
              Array.isArray(checkpoint.options)
                ? (checkpoint.options as string[]).join(", ")
                : ""
            }
            onChange={(event) =>
              onChange({
                options: event.target.value
                  .split(",")
                  .map((value) => value.trim())
                  .filter(Boolean),
              })
            }
            className={inputClass}
            placeholder="aprovar, revisar, cancelar"
          />
        </LabeledField>
      ) : null}
      <LabeledField
        label="Mensagem do checkpoint"
        hint="Aceita variáveis como {{steps.passo-1.summary}}."
      >
        <textarea
          value={String(checkpoint.prompt_template ?? "")}
          onChange={(event) => onChange({ prompt_template: event.target.value })}
          rows={2}
          className="w-full rounded border border-zinc-800 bg-zinc-900/50 p-2 font-mono text-[12px] text-zinc-200 outline-none"
        />
      </LabeledField>
    </div>
  );
}
