import { useEffect, useState } from "react";
import { Loader2, Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Field, FormError, Input, Select } from "@/components/ui/field";
import { Modal } from "@/components/ui/modal";
import { TimezoneSelect } from "@/components/ui/timezone-select";
import { apiPatch } from "@/lib/api";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import type { Workspace } from "@/lib/types";
import { fieldInputClass as inputClass } from "@/lib/ui-classes";
import { cn } from "@/lib/utils";

export function WorkspaceEditModal({
  workspace,
  onClose,
  onSaved,
}: {
  workspace: Workspace | null;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [color, setColor] = useState("#3b82f6");
  const [policy, setPolicy] = useState("COST_BENEFIT");
  const [timezone, setTimezone] = useState("UTC");
  const [email, setEmail] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!workspace) return;
    setName(workspace.name);
    setColor(workspace.color_hex);
    setPolicy(workspace.default_router_policy);
    setTimezone(workspace.timezone);
    setEmail(workspace.default_email_recipient ?? "");
    setError(null);
  }, [workspace]);

  async function save() {
    if (!workspace) return;
    if (!name.trim()) {
      setError("Informe o nome do workspace.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await apiPatch(`/api/workspaces/${workspace.id}`, {
        name: name.trim(),
        color_hex: color,
        default_router_policy: policy,
        timezone: timezone.trim(),
        default_email_recipient: email.trim() || null,
      });
      await onSaved();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      open={!!workspace}
      onClose={onClose}
      title={`Editar ${workspace?.name ?? "workspace"}`}
      description="Esses valores são defaults do workspace ativo. A pasta no disco não é movida."
      size="md"
    >
      <div className="space-y-3">
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Nome" htmlFor="workspace-name">
            <Input
              id="workspace-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </Field>
          <Field label="Cor" htmlFor="workspace-color">
            <div className="flex h-9 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2">
              <input
                id="workspace-color"
                type="color"
                value={color}
                onChange={(event) => setColor(event.target.value)}
                className="h-5 w-7 cursor-pointer rounded border-0 bg-transparent p-0"
              />
              <span className="font-mono text-[12px] text-zinc-500">{color}</span>
            </div>
          </Field>
          <Field label="Política padrão" htmlFor="workspace-policy">
            <Select
              id="workspace-policy"
              value={policy}
              onChange={(event) => setPolicy(event.target.value)}
            >
              {Object.entries(ROUTER_POLICY_LABELS).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Fuso horário" htmlFor="workspace-timezone">
            <TimezoneSelect
              id="workspace-timezone"
              value={timezone}
              onChange={setTimezone}
              className={cn(inputClass, "text-[12px]")}
            />
          </Field>
          <Field
            label="E-mail padrão de notificação"
            htmlFor="workspace-email"
            className="sm:col-span-2"
          >
            <Input
              id="workspace-email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              placeholder="ops@empresa.com"
            />
          </Field>
        </div>
        {error ? <FormError>{error}</FormError> : null}
        <div className="flex justify-end gap-2 border-t border-zinc-800/70 pt-4">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button size="sm" onClick={() => void save()} disabled={saving}>
            {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
            Salvar workspace
          </Button>
        </div>
      </div>
    </Modal>
  );
}
