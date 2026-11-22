import { ArrowLeftRight, Loader2 } from "lucide-react";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, Input } from "@/components/ui/field";
import { ModelCombobox } from "@/components/ui/model-combobox";
import type { AbForm } from "./worktrees-shared";

export function AbCreateCard({
  form,
  setForm,
  creating,
  workspaceId,
  onCreate,
}: {
  form: AbForm;
  setForm: (next: AbForm) => void;
  creating: boolean;
  workspaceId: string;
  onCreate: () => void;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Novo experimento A/B</CardTitle>
        <p className="text-[13px] text-zinc-500">
          Mesmo prompt, dois agentes/modelos e duas worktrees. O vencedor só é
          mesclado após confirmação.
        </p>
      </CardHeader>
      <CardContent className="space-y-3">
        <Field label="Título" htmlFor="ab-title">
          <Input
            id="ab-title"
            value={form.title}
            onChange={(event) => setForm({ ...form, title: event.target.value })}
            placeholder="Ex.: refatorar parser"
          />
        </Field>
        <Field label="Prompt" htmlFor="ab-prompt">
          <textarea
            id="ab-prompt"
            value={form.prompt}
            onChange={(event) => setForm({ ...form, prompt: event.target.value })}
            placeholder="Descreva a tarefa que os dois lados vão executar…"
            className="min-h-[96px] w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2 text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
          />
        </Field>
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Agente A">
            <AgentCombobox
              value={form.agentA}
              onChange={(value) => setForm({ ...form, agentA: value })}
              workspaceId={workspaceId}
              allowEmpty={false}
            />
          </Field>
          <Field label="Agente B">
            <AgentCombobox
              value={form.agentB}
              onChange={(value) => setForm({ ...form, agentB: value })}
              workspaceId={workspaceId}
              allowEmpty={false}
            />
          </Field>
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Modelo A">
            <ModelCombobox
              value={form.modelA || null}
              onChange={(value) => setForm({ ...form, modelA: value ?? "" })}
              allowRouter={false}
              ariaLabel="Modelo A"
            />
          </Field>
          <Field label="Modelo B">
            <ModelCombobox
              value={form.modelB || null}
              onChange={(value) => setForm({ ...form, modelB: value ?? "" })}
              allowRouter={false}
              ariaLabel="Modelo B"
            />
          </Field>
        </div>
        <Button
          size="sm"
          onClick={onCreate}
          disabled={creating}
          className="w-full"
        >
          {creating ? <Loader2 size={13} className="animate-spin" /> : <ArrowLeftRight size={13} />}
          Criar A/B
        </Button>
      </CardContent>
    </Card>
  );
}
