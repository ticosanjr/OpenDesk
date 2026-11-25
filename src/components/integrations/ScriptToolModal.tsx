import { useEffect, useMemo, useState } from "react";
import { Loader2, Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { apiPost, apiPut } from "@/lib/api";
import type { ScriptRunResult, ScriptTool } from "@/lib/types";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import { ScriptToolParamsTab } from "./ScriptToolParamsTab";
import { ScriptToolTestPanel } from "./ScriptToolTestPanel";
import { schemaFields, type SchemaField } from "./script-tool-schema";

type Tab = "script" | "params" | "prompt" | "limits";

const TABS: Array<{ id: Tab; label: string }> = [
  { id: "script", label: "Script" },
  { id: "params", label: "Parâmetros" },
  { id: "prompt", label: "Prompt para o Agente" },
  { id: "limits", label: "Segredos & Limites" },
];

function formState(tool: ScriptTool | null, workspaceId: string) {
  return {
    scope: tool?.workspace_id ? "workspace" : tool ? "global" : "workspace",
    slug: tool?.slug ?? "",
    name: tool?.name ?? "",
    description: tool?.description ?? "",
    agent_prompt: tool?.agent_prompt ?? "",
    runtime: tool?.runtime ?? "bash",
    custom_command: tool?.custom_command ?? "",
    script_body: tool?.script_body ?? "",
    input_schema_json:
      tool?.input_schema_json ?? '{"type":"object","properties":{}}',
    env_secret_refs_json: tool?.env_secret_refs_json ?? "{}",
    timeout_secs: tool?.timeout_secs ?? 30,
    require_approval: tool?.require_approval ?? false,
    enabled: tool?.enabled ?? true,
    workspace_id: workspaceId,
  };
}

export function ScriptToolModal({
  open,
  tool,
  workspaceId,
  runtimes,
  onClose,
  onSaved,
}: {
  open: boolean;
  tool: ScriptTool | null;
  workspaceId: string;
  runtimes: string[];
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const [tab, setTab] = useState<Tab>("script");
  const [form, setForm] = useState(() => formState(tool, workspaceId));
  const [fields, setFields] = useState<SchemaField[]>(() =>
    schemaFields(tool?.input_schema_json ?? ""),
  );
  const [testInput, setTestInput] = useState("{}");
  const [result, setResult] = useState<ScriptRunResult | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setForm(formState(tool, workspaceId));
    setFields(schemaFields(tool?.input_schema_json ?? ""));
    setTab("script");
    setTestInput("{}");
    setResult(null);
    setError(null);
  }, [open, tool, workspaceId]);

  const availableRuntimes = useMemo(
    () => (runtimes.length > 0 ? runtimes : ["bash", "python3", "node", "uv", "custom"]),
    [runtimes],
  );

  function set<K extends keyof ReturnType<typeof formState>>(
    key: K,
    value: ReturnType<typeof formState>[K],
  ) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  function body() {
    return {
      scope: form.scope,
      workspace_id: form.workspace_id,
      slug: form.slug.trim(),
      name: form.name.trim(),
      description: form.description.trim(),
      agent_prompt: form.agent_prompt.trim(),
      runtime: form.runtime,
      custom_command:
        form.runtime === "custom" ? form.custom_command.trim() || null : null,
      script_body: form.script_body,
      input_schema_json: form.input_schema_json,
      env_secret_refs_json: form.env_secret_refs_json,
      timeout_secs: Number(form.timeout_secs) || 30,
      require_approval: form.require_approval,
      enabled: form.enabled,
    };
  }

  function validate(): string | null {
    if (!/^[a-z0-9][a-z0-9_-]{1,48}$/.test(form.slug.trim())) {
      return "Slug inválido: use minúsculas, números, _ ou - (2 a 49 caracteres).";
    }
    if (!form.name.trim()) return "Informe o nome da ferramenta.";
    if (!form.script_body.trim()) return "O script não pode ficar vazio.";
    if (form.runtime === "custom" && !form.custom_command.trim()) {
      return "Runtime custom exige um comando.";
    }
    try {
      JSON.parse(form.input_schema_json);
    } catch {
      return "input_schema_json não é JSON válido.";
    }
    try {
      JSON.parse(form.env_secret_refs_json);
    } catch {
      return "env_secret_refs_json não é JSON válido.";
    }
    return null;
  }

  async function save() {
    const problem = validate();
    if (problem) {
      setError(problem);
      return;
    }
    setBusy("save");
    setError(null);
    try {
      if (tool) {
        await apiPut(`/api/script-tools/${tool.id}`, body());
      } else {
        await apiPost("/api/script-tools", body());
      }
      toast({ tone: "success", title: tool ? "Ferramenta atualizada" : "Ferramenta criada" });
      await onSaved();
      onClose();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function test() {
    if (!tool) return;
    let input: unknown;
    try {
      input = JSON.parse(testInput || "{}");
    } catch {
      setError("A entrada do teste não é JSON válido.");
      return;
    }
    setBusy("test");
    setError(null);
    try {
      setResult(
        await apiPost<ScriptRunResult>(`/api/script-tools/${tool.id}/test`, {
          input,
        }),
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={tool ? `Editar ${tool.name}` : "Nova ferramenta (Script MCP)"}
      description="O script roda com timeout, cwd na raiz do workspace e argumentos validados pelo JSON Schema."
      size="xl"
    >
      <div className="space-y-3">
        <div className="flex flex-wrap gap-1">
          {TABS.map((item) => (
            <button
              key={item.id}
              type="button"
              aria-pressed={tab === item.id}
              onClick={() => setTab(item.id)}
              className={cn(
                "rounded-lg px-2.5 py-1 text-[12.5px]",
                tab === item.id
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200",
              )}
            >
              {item.label}
            </button>
          ))}
        </div>

        {tab === "script" ? (
          <div className="space-y-2">
            <div className="grid gap-2 sm:grid-cols-2">
              <label>
                <span className={labelClass}>Escopo</span>
                <select
                  value={form.scope}
                  onChange={(event) => set("scope", event.target.value)}
                  className={inputClass}
                >
                  <option value="workspace">Workspace atual</option>
                  <option value="global">Global (todos)</option>
                </select>
              </label>
              <label>
                <span className={labelClass}>Runtime</span>
                <select
                  value={form.runtime}
                  onChange={(event) => set("runtime", event.target.value)}
                  className={inputClass}
                >
                  {availableRuntimes.map((runtime) => (
                    <option key={runtime} value={runtime}>
                      {runtime}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                <span className={labelClass}>Slug</span>
                <input
                  value={form.slug}
                  onChange={(event) => set("slug", event.target.value)}
                  placeholder="consultar_estoque"
                  className={cn(inputClass, "font-mono")}
                />
              </label>
              <label>
                <span className={labelClass}>Nome</span>
                <input
                  value={form.name}
                  onChange={(event) => set("name", event.target.value)}
                  placeholder="Consultar estoque"
                  className={inputClass}
                />
              </label>
            </div>
            {form.runtime === "custom" ? (
              <label>
                <span className={labelClass}>Comando custom</span>
                <input
                  value={form.custom_command}
                  onChange={(event) => set("custom_command", event.target.value)}
                  placeholder="/usr/local/bin/minha-ferramenta"
                  className={cn(inputClass, "font-mono")}
                />
              </label>
            ) : null}
            <label>
              <span className={labelClass}>Descrição curta</span>
              <input
                value={form.description}
                onChange={(event) => set("description", event.target.value)}
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Código do script</span>
              <textarea
                value={form.script_body}
                onChange={(event) => set("script_body", event.target.value)}
                rows={12}
                spellCheck={false}
                placeholder={
                  form.runtime === "bash"
                    ? '#!/usr/bin/env bash\ninput=$(cat)\necho "$input"'
                    : "# código"
                }
                className={cn(inputClass, "h-auto resize-y py-2 font-mono text-[12px]")}
              />
            </label>
          </div>
        ) : null}

        {tab === "params" ? (
          <ScriptToolParamsTab
            fields={fields}
            setFields={setFields}
            schema={form.input_schema_json}
            setSchema={(value) => set("input_schema_json", value)}
          />
        ) : null}

        {tab === "prompt" ? (
          <label>
            <span className={labelClass}>Instruções para o agente</span>
            <textarea
              value={form.agent_prompt}
              onChange={(event) => set("agent_prompt", event.target.value)}
              rows={10}
              placeholder="Quando usar esta ferramenta, quais dados enviar e como interpretar a saída."
              className={cn(inputClass, "h-auto resize-y py-2 text-[12.5px]")}
            />
            <p className="mt-1 text-[11.5px] text-zinc-500">
              Este texto entra na descrição da tool MCP e no resumo de ferramentas do prompt.
            </p>
          </label>
        ) : null}

        {tab === "limits" ? (
          <div className="space-y-2">
            <div className="grid gap-2 sm:grid-cols-2">
              <label>
                <span className={labelClass}>Timeout (segundos)</span>
                <input
                  type="number"
                  min={1}
                  max={300}
                  value={form.timeout_secs}
                  onChange={(event) => set("timeout_secs", Number(event.target.value))}
                  className={inputClass}
                />
              </label>
              <div className="flex flex-col justify-end gap-1 text-[12.5px] text-zinc-400">
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={form.require_approval}
                    onChange={(event) => set("require_approval", event.target.checked)}
                    className="accent-blue-500"
                  />
                  Exigir aprovação HITL
                </label>
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={form.enabled}
                    onChange={(event) => set("enabled", event.target.checked)}
                    className="accent-blue-500"
                  />
                  Ferramenta habilitada
                </label>
              </div>
            </div>
            <label>
              <span className={labelClass}>env_secret_refs_json</span>
              <textarea
                value={form.env_secret_refs_json}
                onChange={(event) => set("env_secret_refs_json", event.target.value)}
                rows={6}
                spellCheck={false}
                placeholder='{"API_KEY":"minha-chave"}'
                className={cn(inputClass, "h-auto resize-y py-2 font-mono text-[12px]")}
              />
              <p className="mt-1 text-[11.5px] text-zinc-500">
                Mapa ENV_VAR → secret_ref do SecretStore. Valores diretos são recusados.
              </p>
            </label>
          </div>
        ) : null}

        <ScriptToolTestPanel
          input={testInput}
          setInput={setTestInput}
          result={result}
          busy={busy === "test"}
          disabled={!tool}
          onTest={() => void test()}
        />

        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            {error}
          </p>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button size="sm" disabled={busy === "save"} onClick={() => void save()}>
            {busy === "save" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Save size={13} />
            )}
            Salvar
          </Button>
        </div>
      </div>
    </Modal>
  );
}
