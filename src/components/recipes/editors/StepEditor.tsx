import { useState } from "react";
import { inputClass, LabeledField } from "../recipe-fields";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { PromptAttachments } from "@/components/ui/prompt-attachments";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import type { Dictionary } from "../recipe-types";
import { SchemaContractEditor } from "./SchemaContractEditor";

export function StepEditor({
  step,
  workspaceId,
  onChange,
}: {
  step: Dictionary;
  workspaceId?: string | null;
  onChange: (value: Dictionary) => void;
}) {
  const [attachments, setAttachments] = useState<string[]>([]);

  function updateAttachments(paths: string[]) {
    let prompt = String(step.prompt ?? "");
    for (const path of paths.filter((item) => !attachments.includes(item))) {
      prompt += `\n\nArquivos anexados:\n- ${path}`;
    }
    for (const path of attachments.filter((item) => !paths.includes(item))) {
      prompt = prompt
        .replace(`\n\nArquivos anexados:\n- ${path}`, "")
        .replace(`\n- ${path}`, "");
    }
    setAttachments(paths);
    onChange({ prompt });
  }

  return (
    <div className="mt-1.5 space-y-1.5">
      <div className="grid gap-2 sm:grid-cols-2">
        <LabeledField label="Modo">
          <select
            value={String(step.mode ?? "build")}
            onChange={(event) => onChange({ mode: event.target.value })}
            className={inputClass}
          >
            <option value="build">build — executar</option>
            <option value="plan">plan — só planejar</option>
            <option value="audit">audit — revisar</option>
          </select>
        </LabeledField>
        <LabeledField label="Agente">
          <AgentCombobox
            value={String(step.agent ?? "")}
            onChange={(value) => onChange({ agent: value || null })}
            workspaceId={workspaceId}
            placeholder="automático (router)"
            compact
          />
        </LabeledField>
        <div className="sm:col-span-2">
          <LabeledField label="Modelo">
            <ModelCombobox
              value={typeof step.model === "string" ? step.model : null}
              onChange={(value) => onChange({ model: value })}
            />
          </LabeledField>
        </div>
      </div>
      <LabeledField
        label="Executar quando (opcional)"
        hint='Ex.: steps.plan.structured.status == "ok" ou trigger.approved'
      >
        <input
          value={String(step.when ?? "")}
          onChange={(event) => onChange({ when: event.target.value || null })}
          placeholder="vazio = sempre executar"
          className={`${inputClass} font-mono text-[12px]`}
        />
      </LabeledField>
      <div className="grid gap-2 sm:grid-cols-2">
        <LabeledField label="Tentativas extras (retry_max)">
          <input
            type="number"
            min={0}
            max={5}
            value={step.retry_max == null ? "" : Number(step.retry_max)}
            onChange={(event) =>
              onChange({
                retry_max: event.target.value ? Number(event.target.value) : null,
              })
            }
            className={inputClass}
          />
        </LabeledField>
        <LabeledField label="Timeout do step (segundos)">
          <input
            type="number"
            min={1}
            max={7200}
            value={step.timeout_secs == null ? "" : Number(step.timeout_secs)}
            onChange={(event) =>
              onChange({
                timeout_secs: event.target.value ? Number(event.target.value) : null,
              })
            }
            className={inputClass}
          />
        </LabeledField>
      </div>
      <LabeledField
        label="Falha de schema"
        hint="retry_format tenta de novo; hitl pede decisão humana; fail interrompe."
      >
        <select
          value={String(step.on_schema_error ?? "retry_format")}
          onChange={(event) =>
            onChange({ on_schema_error: event.target.value })
          }
          className={inputClass}
        >
          <option value="retry_format">retry_format</option>
          <option value="hitl">hitl</option>
          <option value="fail">fail</option>
        </select>
      </LabeledField>
      <LabeledField
        label="Vault refs (separados por vírgula)"
        hint="Ids de credenciais já cadastradas; valide antes de rodar."
      >
        <input
          value={
            Array.isArray(step.vault_refs) ? step.vault_refs.join(", ") : ""
          }
          onChange={(event) => {
            const refs = event.target.value
              .split(",")
              .map((value) => value.trim())
              .filter(Boolean);
            onChange({ vault_refs: refs });
          }}
          placeholder="stripe-live, portal-fornecedor"
          className={`${inputClass} font-mono text-[12px]`}
        />
      </LabeledField>
      <SchemaContractEditor
        input={step.input_schema}
        output={step.output_schema}
        onChange={(value) => onChange(value)}
      />
      <LabeledField label="Prompt da etapa">
        <SlashTextarea
          value={String(step.prompt ?? "")}
          onChange={(value) => onChange({ prompt: value })}
          workspaceId={workspaceId}
          rows={4}
          className="font-mono text-[12px]"
        />
        <PromptAttachments
          workspaceId={workspaceId}
          value={attachments}
          onChange={updateAttachments}
          className="mt-2"
        />
      </LabeledField>
    </div>
  );
}
