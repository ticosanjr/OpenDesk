import { useEffect, useState } from "react";
import { inputClass, LabeledField } from "../recipe-fields";
import type { Dictionary, FlowEntry } from "../recipe-types";

export function DagNodeEditor({
  node,
  onChange,
}: {
  node: FlowEntry;
  onChange: (value: Dictionary) => void;
}) {
  const type = node.type;
  return (
    <div className="mt-1.5 space-y-1.5">
      <LabeledField label="Id do nó">
        <input
          value={String(node.id ?? "")}
          onChange={(event) => onChange({ id: event.target.value })}
          className={`${inputClass} font-mono text-[12px]`}
        />
      </LabeledField>

      {type === "branch" ? (
        <>
          <LabeledField label="Condição (when)" hint="Expressão avaliada pelo motor de receitas.">
            <input
              value={String(node.when ?? "")}
              onChange={(event) => onChange({ when: event.target.value })}
              className={`${inputClass} font-mono text-[12px]`}
            />
          </LabeledField>
          <JsonField
            label="then (lista de nós)"
            value={node.then}
            onChange={(value) => onChange({ then: value })}
          />
          <JsonField
            label="else (lista de nós)"
            value={node.otherwise ?? node.else}
            onChange={(value) => onChange({ else: value })}
          />
        </>
      ) : null}

      {type === "parallel" ? (
        <>
          <LabeledField label="Join">
            <select
              value={String(node.join ?? "all")}
              onChange={(event) => onChange({ join: event.target.value })}
              className={inputClass}
            >
              <option value="all">all — todos os ramos</option>
              <option value="any">any — primeiro que concluir</option>
            </select>
          </LabeledField>
          <JsonField
            label="branches (array de listas)"
            value={node.branches}
            onChange={(value) => onChange({ branches: value })}
            rows={5}
          />
        </>
      ) : null}

      {type === "map" ? (
        <>
          <LabeledField label="Itens (expressão)">
            <input
              value={String(node.items ?? "")}
              onChange={(event) => onChange({ items: event.target.value })}
              placeholder="{{steps.listar.structured.items}}"
              className={`${inputClass} font-mono text-[12px]`}
            />
          </LabeledField>
          <div className="grid gap-2 sm:grid-cols-2">
            <LabeledField label="Variável do item">
              <input
                value={String(node.item_var ?? "item")}
                onChange={(event) => onChange({ item_var: event.target.value })}
                className={inputClass}
              />
            </LabeledField>
            <LabeledField label="Concorrência máxima">
              <input
                type="number"
                min={1}
                max={8}
                value={Number(node.max_concurrency ?? 3)}
                onChange={(event) =>
                  onChange({ max_concurrency: Number(event.target.value) })
                }
                className={inputClass}
              />
            </LabeledField>
          </div>
          <JsonField
            label="flow (nós executados por item)"
            value={node.flow}
            onChange={(value) => onChange({ flow: value })}
            rows={5}
          />
        </>
      ) : null}

      {type === "recipe_call" ? (
        <>
          <LabeledField label="Receita chamada">
            <input
              value={String(node.recipe ?? "")}
              onChange={(event) => onChange({ recipe: event.target.value })}
              className={`${inputClass} font-mono text-[12px]`}
            />
          </LabeledField>
          <JsonField
            label="with (entradas da sub-receita)"
            value={node.with}
            onChange={(value) => onChange({ with: value })}
            emptyValue={{}}
          />
          <div className="grid gap-2 sm:grid-cols-2">
            <LabeledField label="Budget USD (opcional)">
              <input
                type="number"
                min={0}
                step="0.01"
                value={node.budget_usd == null ? "" : Number(node.budget_usd)}
                onChange={(event) =>
                  onChange({
                    budget_usd: event.target.value ? Number(event.target.value) : null,
                  })
                }
                className={inputClass}
              />
            </LabeledField>
            <LabeledField label="Condição (when)">
              <input
                value={String(node.when ?? "")}
                onChange={(event) => onChange({ when: event.target.value || null })}
                className={`${inputClass} font-mono text-[12px]`}
              />
            </LabeledField>
          </div>
        </>
      ) : null}

      {type === "loop" ? (
        <>
          <div className="grid gap-2 sm:grid-cols-2">
            <LabeledField label="Iterações máximas">
              <input
                type="number"
                min={1}
                max={100}
                value={Number(node.max_iterations ?? 3)}
                onChange={(event) =>
                  onChange({ max_iterations: Number(event.target.value) })
                }
                className={inputClass}
              />
            </LabeledField>
            <LabeledField label="Até (until, opcional)">
              <input
                value={String(node.until ?? "")}
                onChange={(event) => onChange({ until: event.target.value || null })}
                className={`${inputClass} font-mono text-[12px]`}
              />
            </LabeledField>
          </div>
          <JsonField
            label="flow (corpo do loop)"
            value={node.flow}
            onChange={(value) => onChange({ flow: value })}
            rows={5}
          />
        </>
      ) : null}

      {type === "switch" ? (
        <>
          <JsonField
            label="cases (lista de when + flow)"
            value={node.cases}
            onChange={(value) => onChange({ cases: value })}
            rows={5}
          />
          <JsonField
            label="default (lista de nós)"
            value={node.default ?? node.otherwise}
            onChange={(value) => onChange({ default: value })}
            rows={4}
          />
        </>
      ) : null}
    </div>
  );
}

function JsonField({
  label,
  value,
  onChange,
  rows = 4,
  emptyValue = [],
}: {
  label: string;
  value: unknown;
  onChange: (value: unknown) => void;
  rows?: number;
  emptyValue?: unknown;
}) {
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setText(value == null ? "" : JSON.stringify(value, null, 2));
  }, [value]);

  function commit() {
    if (!text.trim()) {
      setError(null);
      onChange(emptyValue);
      return;
    }
    try {
      const parsed = JSON.parse(text);
      if (parsed == null || typeof parsed !== "object") {
        throw new Error("use um objeto ou array JSON");
      }
      setError(null);
      onChange(parsed);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return (
    <LabeledField label={label} hint={error ?? undefined}>
      <textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        onBlur={commit}
        rows={rows}
        className={`${inputClass} font-mono text-[11.5px] ${error ? "border-red-500/60" : ""}`}
      />
    </LabeledField>
  );
}
