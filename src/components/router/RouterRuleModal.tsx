import { useEffect, useState } from "react";
import { Loader2, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Field, FormError, Input, Select, Textarea } from "@/components/ui/field";
import { Modal } from "@/components/ui/modal";
import { apiPatch, apiPost } from "@/lib/api";
import type { RouterRule } from "@/lib/types";

export function RouterRuleModal({
  open,
  workspaceId,
  rule,
  onClose,
  onCreated,
}: {
  open: boolean;
  workspaceId: string;
  rule?: RouterRule | null;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [condition, setCondition] = useState(
    '{"field":"tags","op":"contains","value":"roteiro"}',
  );
  const [mode, setMode] = useState("build");
  const [agent, setAgent] = useState("build");
  const [model, setModel] = useState("opencode-go/glm-5.3");
  const [effort, setEffort] = useState("high");
  const [priority, setPriority] = useState(10);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setName(rule?.name ?? "");
    setCondition(
      rule?.condition_json ??
        '{"field":"tags","op":"contains","value":"roteiro"}',
    );
    setMode(rule?.target_mode ?? "build");
    setAgent(rule?.target_agent ?? "build");
    setModel(rule?.target_model ?? "opencode-go/glm-5.3");
    setEffort(rule?.target_effort ?? "high");
    setPriority(rule?.priority_order ?? 10);
    setError(null);
  }, [open, rule]);

  async function submit() {
    if (!name.trim()) {
      setError("Informe o nome da regra.");
      return;
    }
    try {
      JSON.parse(condition);
    } catch (cause) {
      setError(
        `condition_json inválido: ${cause instanceof Error ? cause.message : cause}`,
      );
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const payload = {
        workspace_id: workspaceId,
        name: name.trim(),
        priority_order: priority,
        condition_json: condition,
        target_mode: mode,
        target_agent: agent.trim() || null,
        target_model: model.trim(),
        target_effort: effort,
        is_enabled: true,
      };
      if (rule) {
        await apiPatch(`/api/router/rules/${rule.id}`, payload);
      } else {
        await apiPost("/api/router/rules", payload);
      }
      setName("");
      await onCreated();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={rule ? "Editar regra do roteador" : "Nova regra do roteador"}
      description="A condição usa field/op/value. Combine com all/any para regras compostas."
      size="lg"
    >
      <div className="grid gap-3 md:grid-cols-2">
        <Field label="Nome" htmlFor="rule-name" className="md:col-span-2">
          <Input
            id="rule-name"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </Field>
        <Field label="Condição JSON" htmlFor="rule-condition" className="md:col-span-2">
          <Textarea
            id="rule-condition"
            value={condition}
            onChange={(event) => setCondition(event.target.value)}
            rows={3}
            className="min-h-0 font-mono text-[12px]"
          />
        </Field>
        <Field label="Prioridade" htmlFor="rule-priority">
          <Input
            id="rule-priority"
            type="number"
            value={priority}
            onChange={(event) => setPriority(Number(event.target.value))}
          />
        </Field>
        <Field label="Modo alvo" htmlFor="rule-mode">
          <Select
            id="rule-mode"
            value={mode}
            onChange={(event) => setMode(event.target.value)}
          >
            <option value="plan">plan</option>
            <option value="build">build</option>
            <option value="plan_then_build">plan_then_build</option>
          </Select>
        </Field>
        <Field label="Agente" htmlFor="rule-agent">
          <Input
            id="rule-agent"
            value={agent}
            onChange={(event) => setAgent(event.target.value)}
          />
        </Field>
        <Field label="Modelo" htmlFor="rule-model">
          <Input
            id="rule-model"
            value={model}
            onChange={(event) => setModel(event.target.value)}
          />
        </Field>
        <Field label="Effort" htmlFor="rule-effort">
          <Select
            id="rule-effort"
            value={effort}
            onChange={(event) => setEffort(event.target.value)}
          >
            <option value="low">low</option>
            <option value="medium">medium</option>
            <option value="high">high</option>
          </Select>
        </Field>
      </div>
      {error ? (
        <div className="mt-3">
          <FormError>{error}</FormError>
        </div>
      ) : null}
      <div className="mt-4 flex justify-end gap-2 border-t border-zinc-800/70 pt-4">
        <Button variant="ghost" size="sm" onClick={onClose}>
          Cancelar
        </Button>
        <Button size="sm" onClick={() => void submit()} disabled={saving}>
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          {rule ? "Salvar regra" : "Criar regra"}
        </Button>
      </div>
    </Modal>
  );
}
