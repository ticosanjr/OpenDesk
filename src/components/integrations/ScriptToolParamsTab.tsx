import { Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { fieldInputClass as inputClass, fieldLabelClass as labelClass } from "@/lib/ui-classes";
import {
  RUNTIME_TYPES,
  fieldsSchema,
  type SchemaField,
} from "./script-tool-schema";

export function ScriptToolParamsTab({
  fields,
  setFields,
  schema,
  setSchema,
}: {
  fields: SchemaField[];
  setFields: (update: (current: SchemaField[]) => SchemaField[]) => void;
  schema: string;
  setSchema: (schema: string) => void;
}) {
  return (
    <div className="space-y-2">
      <p className="text-[12px] text-zinc-500">
        Construtor simples de campos; o JSON Schema completo continua editável abaixo.
      </p>
      {fields.map((field, index) => (
        <div key={index} className="flex flex-wrap items-end gap-1.5">
          <input
            value={field.name}
            onChange={(event) =>
              setFields((current) =>
                current.map((item, itemIndex) =>
                  itemIndex === index ? { ...item, name: event.target.value } : item,
                ),
              )
            }
            placeholder="nome"
            className={cn(inputClass, "min-w-[140px] flex-1 font-mono")}
          />
          <select
            value={field.type}
            onChange={(event) =>
              setFields((current) =>
                current.map((item, itemIndex) =>
                  itemIndex === index ? { ...item, type: event.target.value } : item,
                ),
              )
            }
            className={cn(inputClass, "w-28")}
          >
            {RUNTIME_TYPES.map((type) => (
              <option key={type} value={type}>
                {type}
              </option>
            ))}
          </select>
          <label className="flex h-8 items-center gap-1 text-[12px] text-zinc-400">
            <input
              type="checkbox"
              checked={field.required}
              onChange={(event) =>
                setFields((current) =>
                  current.map((item, itemIndex) =>
                    itemIndex === index
                      ? { ...item, required: event.target.checked }
                      : item,
                  ),
                )
              }
              className="accent-blue-500"
            />
            obrigatório
          </label>
          <Button
            variant="ghost"
            size="sm"
            className="h-8 text-red-300/80"
            onClick={() =>
              setFields((current) => current.filter((_, itemIndex) => itemIndex !== index))
            }
          >
            <Trash2 size={12} />
          </Button>
        </div>
      ))}
      <div className="flex flex-wrap gap-1.5">
        <Button
          variant="outline"
          size="sm"
          onClick={() =>
            setFields((current) => [
              ...current,
              { name: "", type: "string", required: false },
            ])
          }
        >
          <Plus size={12} />
          Campo
        </Button>
        <Button variant="secondary" size="sm" onClick={() => setSchema(fieldsSchema(fields))}>
          Aplicar ao schema
        </Button>
      </div>
      <label>
        <span className={labelClass}>input_schema_json</span>
        <textarea
          value={schema}
          onChange={(event) => setSchema(event.target.value)}
          rows={8}
          spellCheck={false}
          className={cn(inputClass, "h-auto resize-y py-2 font-mono text-[12px]")}
        />
      </label>
    </div>
  );
}
