export type SchemaField = { name: string; type: string; required: boolean };

export const RUNTIME_TYPES = ["string", "number", "integer", "boolean", "object", "array"];

export function schemaFields(schema: string): SchemaField[] {
  try {
    const parsed = JSON.parse(schema) as {
      properties?: Record<string, { type?: string }>;
      required?: string[];
    };
    return Object.entries(parsed.properties ?? {}).map(([name, value]) => ({
      name,
      type: value.type ?? "string",
      required: (parsed.required ?? []).includes(name),
    }));
  } catch {
    return [];
  }
}

export function fieldsSchema(fields: SchemaField[]): string {
  const properties = Object.fromEntries(
    fields
      .filter((field) => field.name.trim())
      .map((field) => [field.name.trim(), { type: field.type }]),
  );
  const required = fields
    .filter((field) => field.required && field.name.trim())
    .map((field) => field.name.trim());
  return JSON.stringify({ type: "object", properties, required }, null, 2);
}
