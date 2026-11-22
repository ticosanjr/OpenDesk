import { LabeledField, inputClass } from "../recipe-fields";
import { SINK_TYPES } from "./recipe-editor-model";
import { cn } from "@/lib/utils";
import type { Dictionary } from "../recipe-types";

export function SinkEditor({
  sink,
  onChange,
}: {
  sink: Dictionary;
  onChange: (value: Dictionary) => void;
}) {
  const config = (sink.config ?? {}) as Dictionary;
  const sinkType = String(sink.sink_type ?? "DOC_COMPILER");
  function patchConfig(patch: Dictionary) {
    onChange({ config: { ...config, ...patch } });
  }
  return (
    <div className="mt-1.5 space-y-1.5">
      {sinkType === "EMAIL_SMTP" ? (
        <div className="grid gap-1.5 sm:grid-cols-2">
          <LabeledField
            label="Para"
            hint="Vazio usa o e-mail padrão do workspace."
          >
            <input
              value={String(config.to ?? "")}
              onChange={(event) => patchConfig({ to: event.target.value })}
              placeholder="cliente@empresa.com"
              className={inputClass}
            />
          </LabeledField>
          <LabeledField label="Assunto">
            <input
              value={String(config.subject ?? "")}
              onChange={(event) => patchConfig({ subject: event.target.value })}
              placeholder="[OpenDesk] Resultado"
              className={inputClass}
            />
          </LabeledField>
          <LabeledField
            label="Mensagem extra"
            hint="Aparece antes do resumo do run no corpo do e-mail."
          >
            <textarea
              value={String(config.body ?? "")}
              onChange={(event) => patchConfig({ body: event.target.value })}
              rows={3}
              placeholder="Segue o resultado processado pela receita."
              className={cn(inputClass, "h-auto py-2")}
            />
          </LabeledField>
          <label className="flex items-center gap-2 self-end pb-1 text-[12.5px] text-zinc-400">
            <input
              type="checkbox"
              checked={config.attach_artifacts !== false}
              onChange={(event) =>
                patchConfig({ attach_artifacts: event.target.checked })
              }
              className="accent-blue-500"
            />
            anexar artefatos (até 5)
          </label>
        </div>
      ) : null}
      {sinkType === "TELEGRAM_MSG" ? (
        <div className="grid gap-1.5 sm:grid-cols-2">
          <LabeledField
            label="Chat ID"
            hint="Use um chat que já tenha enviado /start ao bot."
          >
            <input
              value={String(config.chat_id ?? "")}
              onChange={(event) => patchConfig({ chat_id: event.target.value })}
              placeholder="123456789"
              className={inputClass}
            />
          </LabeledField>
          <LabeledField
            label="Segredo do token"
            hint="Nome do segredo em Integrações; o padrão é telegram-token."
          >
            <input
              value={String(config.token_ref ?? "telegram-token")}
              onChange={(event) => patchConfig({ token_ref: event.target.value })}
              className={inputClass}
            />
          </LabeledField>
          <label className="flex items-center gap-2 self-end pb-1 text-[12.5px] text-zinc-400 sm:col-span-2">
            <input
              type="checkbox"
              checked={config.attach_artifacts !== false}
              onChange={(event) =>
                patchConfig({ attach_artifacts: event.target.checked })
              }
              className="accent-blue-500"
            />
            enviar artefatos como arquivos (até 5)
          </label>
        </div>
      ) : null}
      <div className="grid gap-1.5 sm:grid-cols-2">
        <LabeledField label="Tipo de saída">
          <select
            value={String(sink.sink_type ?? "DOC_COMPILER")}
            onChange={(event) => onChange({ sink_type: event.target.value })}
            className={inputClass}
          >
            {SINK_TYPES.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </LabeledField>
        <LabeledField
          label={sinkType === "EMAIL_SMTP" ? "Avançado (JSON)" : "Configuração (JSON)"}
          hint="Caminhos e destinatários aceitam variáveis do fluxo."
        >
          <input
            value={JSON.stringify(config)}
            onChange={(event) => {
              try {
                onChange({ config: JSON.parse(event.target.value) });
              } catch {
                // Keep the last valid JSON while the user types.
              }
            }}
            className={cn(inputClass, "font-mono")}
            placeholder="{}"
          />
        </LabeledField>
      </div>
    </div>
  );
}
