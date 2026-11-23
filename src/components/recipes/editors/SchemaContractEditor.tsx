import { useEffect, useState } from "react";
import { inputClass, LabeledField } from "../recipe-fields";
import type { Dictionary } from "../recipe-types";

export function SchemaContractEditor({
  input,
  output,
  onChange,
}: {
  input: unknown;
  output: unknown;
  onChange: (value: {
    input_schema?: Dictionary | null;
    output_schema?: Dictionary | null;
  }) => void;
}) {
  return (
    <>
      <SchemaField
        label="Schema de entrada (JSON, opcional)"
        value={input}
        onChange={(value) => onChange({ input_schema: value })}
      />
      <SchemaField
        label="Schema de saída (JSON, opcional)"
        hint="O agente deve responder apenas com JSON válido; falhas seguem on_schema_error."
        value={output}
        onChange={(value) => onChange({ output_schema: value })}
      />
    </>
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
