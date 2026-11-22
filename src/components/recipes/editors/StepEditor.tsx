import { useEffect, useState } from "react";
import { inputClass, LabeledField } from "../recipe-fields";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { PromptAttachments } from "@/components/ui/prompt-attachments";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import type { Dictionary } from "../recipe-types";

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
      <SchemaField
        label="Schema de entrada (JSON, opcional)"
        value={step.input_schema}
        onChange={(value) => onChange({ input_schema: value })}
      />
      <SchemaField
        label="Schema de saída (JSON, opcional)"
        hint="O agente deve responder apenas com JSON válido; falhas geram retry automático."
        value={step.output_schema}
        onChange={(value) => onChange({ output_schema: value })}
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

function SchemaField({
  label,
  hint,
  value,
  onChange,
}: {
  label: string;
  hint?: string;
  value: unknown;
  onChange: (value: Dictionary | null) => void;
}) {
  const initial =
    value && typeof value === "object" ? JSON.stringify(value, null, 2) : "";
  const [text, setText] = useState(initial);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setText(value && typeof value === "object" ? JSON.stringify(value, null, 2) : "");
  }, [value]);

  function commit() {
    if (!text.trim()) {
      setError(null);
      onChange(null);
      return;
    }
    try {
      const parsed = JSON.parse(text) as Dictionary;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        throw new Error("o schema precisa ser um objeto JSON");
      }
      setError(null);
      onChange(parsed);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return (
    <LabeledField label={label} hint={error ?? hint}>
      <textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        onBlur={commit}
        rows={3}
        placeholder='{ "type": "object", "required": [] }'
        className={`${inputClass} font-mono text-[11.5px] ${error ? "border-red-500/60" : ""}`}
      />
    </LabeledField>
  );
}
