import { useRef, useState } from "react";
import { ChevronDown, ChevronUp, Plus, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { IconLabel } from "@/components/ui/icon-label";
import { FieldHint } from "@/components/ui/field-hint";
import { PromptAttachments } from "@/components/ui/prompt-attachments";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { cn } from "@/lib/utils";
import { inputClass, LabeledField } from "./recipe-fields";
import { TriggerEditor } from "./recipe-trigger-editor";
import type { Dictionary, FlowEntry } from "./recipe-types";

export type { Dictionary } from "./recipe-types";

export const HITL_TYPES = [
  {
    value: "BINARY_GATE",
    label: "Aprovação simples (BINARY_GATE)",
    description: "Uma decisão de aprovar ou rejeitar.",
  },
  {
    value: "MULTI_CHOICE",
    label: "Escolha uma opção (MULTI_CHOICE)",
    description: "O usuário escolhe uma das opções informadas.",
  },
  {
    value: "APPROVAL_WITH_FEEDBACK",
    label: "Aprovar com feedback (APPROVAL_WITH_FEEDBACK)",
    description: "Aprova, rejeita ou pede uma correção com texto.",
  },
  {
    value: "ARTIFACT_EDIT_HANDOFF",
    label: "Editar arquivo (ARTIFACT_EDIT_HANDOFF)",
    description: "O usuário edita o artefato e devolve o arquivo.",
  },
  {
    value: "TOOL_PERMISSION_PROXY",
    label: "Permissão de ferramenta (TOOL_PERMISSION_PROXY)",
    description: "Autoriza uma ferramenta externa antes de continuar.",
  },
];

export const SINK_TYPES = [
  { value: "DOC_COMPILER", label: "Compilar documento (DOC_COMPILER)" },
  { value: "EMAIL_SMTP", label: "Enviar e-mail (EMAIL_SMTP)" },
  { value: "SIGNAL_MSG", label: "Enviar Signal (SIGNAL_MSG)" },
  { value: "TELEGRAM_MSG", label: "Enviar Telegram (TELEGRAM_MSG)" },
  { value: "RSYNC_FS", label: "Copiar via Rsync (RSYNC_FS)" },
  { value: "GIT_COMMIT", label: "Commit no Git (GIT_COMMIT)" },
  { value: "SQL_INSERT", label: "Inserir no SQL (SQL_INSERT)" },
];

const CHANNELS = ["DESKTOP_UI", "EMAIL", "SIGNAL"];

const ON_TIMEOUT = [
  { value: "PAUSE_AND_ALERT", label: "Pausar e avisar" },
  { value: "AUTO_APPROVE", label: "Aprovar automaticamente" },
  { value: "ABORT", label: "Abortar a receita" },
];


export function LocalYamlEditor({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const textarea = useRef<HTMLTextAreaElement | null>(null);
  const gutter = useRef<HTMLDivElement | null>(null);
  const lines = value.split("\n").length;

  return (
    <div className="flex h-full min-h-0 bg-zinc-950">
      <div
        ref={gutter}
        aria-hidden
        className="h-full select-none overflow-hidden border-r border-zinc-800/70 px-2 py-3 text-right font-mono text-[10px] leading-[1.6] text-zinc-700"
      >
        {Array.from({ length: lines }).map((_, index) => (
          <div key={index}>{index + 1}</div>
        ))}
      </div>
      <textarea
        ref={textarea}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onScroll={(event) => {
          if (gutter.current) {
            gutter.current.scrollTop = event.currentTarget.scrollTop;
          }
        }}
        spellCheck={false}
        className="h-full min-h-0 flex-1 resize-none bg-transparent p-3 font-mono text-[11px] leading-[1.6] text-zinc-200 outline-none"
        aria-label="Editor YAML da receita"
      />
    </div>
  );
}

function isFlowEntry(value: unknown): value is FlowEntry {
  if (!value || typeof value !== "object") return false;
  const type = (value as Dictionary).type;
  return type === "step" || type === "hitl" || type === "sink";
}

function normalizeFlow(recipe: Dictionary): FlowEntry[] {
  if (Array.isArray(recipe.flow)) {
    const flow = recipe.flow.filter(isFlowEntry);
    if (flow.length > 0) return flow;
  }
  const steps = Array.isArray(recipe.steps)
    ? (recipe.steps as Dictionary[])
    : [];
  const checkpoints = Array.isArray(recipe.hitl_checkpoints)
    ? (recipe.hitl_checkpoints as Dictionary[])
    : [];
  const sinks = Array.isArray(recipe.sinks)
    ? (recipe.sinks as Dictionary[])
    : [];
  const flow: FlowEntry[] = [];
  for (const step of steps) {
    flow.push({ ...step, type: "step" });
    for (const checkpoint of checkpoints.filter(
      (item) => item.after_step === step.id,
    )) {
      flow.push({ ...checkpoint, type: "hitl" });
    }
  }
  for (const checkpoint of checkpoints.filter(
    (item) => !item.after_step,
  )) {
    flow.push({ ...checkpoint, type: "hitl" });
  }
  for (const sink of sinks) {
    flow.push({
      ...sink,
      type: "sink",
      sink_type: sink.sink_type ?? sink.type,
    });
  }
  return flow;
}

function normalizeTriggers(recipe: Dictionary): Dictionary[] {
  if (Array.isArray(recipe.triggers)) {
    return (recipe.triggers as Dictionary[]).filter(
      (trigger) => trigger && typeof trigger === "object",
    );
  }
  const legacy = recipe.trigger;
  if (legacy && typeof legacy === "object") {
    const trigger = legacy as Dictionary;
    if (typeof trigger.type === "string" && trigger.type) return [trigger];
  }
  return [];
}

function commit(
  recipe: Dictionary,
  flow: FlowEntry[],
  triggers: Dictionary[],
  onChange: (next: Dictionary) => void,
) {
  const next = { ...recipe };
  delete next.steps;
  delete next.hitl_checkpoints;
  delete next.sinks;
  delete next.trigger;
  next.flow = flow;
  next.triggers = triggers;
  onChange(next);
}

export function RecipeStepper({
  recipe,
  workspaceId,
  onChange,
}: {
  recipe: Dictionary;
  workspaceId?: string | null;
  onChange: (next: Dictionary) => void;
}) {
  const flow = normalizeFlow(recipe);
  const triggers = normalizeTriggers(recipe);

  function setFlow(next: FlowEntry[]) {
    commit(recipe, next, triggers, onChange);
  }

  function setTriggers(next: Dictionary[]) {
    commit(recipe, flow, next, onChange);
  }

  function move(index: number, direction: -1 | 1) {
    const target = index + direction;
    if (target < 0 || target >= flow.length) return;
    const next = [...flow];
    [next[index], next[target]] = [next[target], next[index]];
    setFlow(next);
  }

  function patch(index: number, value: Dictionary) {
    setFlow(
      flow.map((item, position) =>
        position === index ? ({ ...item, ...value } as FlowEntry) : item,
      ),
    );
  }

  function addStep() {
    const count = flow.filter((item) => item.type === "step").length + 1;
    setFlow([
      ...flow,
      {
        type: "step",
        id: `passo-${count}`,
        name: `Passo ${count}`,
        mode: "build",
        prompt: "Descreva o que esta etapa deve fazer.",
      },
    ]);
  }

  function addCheckpoint() {
    const count = flow.filter((item) => item.type === "hitl").length + 1;
    setFlow([
      ...flow,
      {
        type: "hitl",
        id: `checkpoint-${count}`,
        name: `Checkpoint ${count}`,
        interaction_type: "BINARY_GATE",
        channels: ["DESKTOP_UI"],
        timeout_minutes: 240,
        on_timeout: "PAUSE_AND_ALERT",
        prompt_template: "Aprovar a execução?",
      },
    ]);
  }

  function addSink() {
    const count = flow.filter((item) => item.type === "sink").length + 1;
    setFlow([
      ...flow,
      {
        type: "sink",
        sink_type: "DOC_COMPILER",
        name: `Saída ${count}`,
        config: { format: "pdf" },
      },
    ]);
  }

  return (
    <div className="space-y-3">
      <section className="rounded-lg border border-blue-500/30 bg-blue-500/[0.04] p-2.5">
        <div className="flex flex-wrap items-center gap-2">
          <p className="text-[10px] font-semibold uppercase tracking-wide text-blue-200">
            Gatilhos de entrada
          </p>
          <CountPill value={triggers.length} tone="blue" />
          <FieldHint hint="Uma receita pode ter zero gatilhos (só por rotina/manual) ou vários ao mesmo tempo." />
          <Button
            variant="outline"
            size="sm"
            className="ml-auto h-7"
            onClick={() => setTriggers([...triggers, { type: "MANUAL", config: {} }])}
          >
            <IconLabel icon={<Plus size={11} />}>Adicionar gatilho</IconLabel>
          </Button>
        </div>
        {triggers.length === 0 ? (
          <p className="mt-2 rounded border border-dashed border-zinc-700 px-2.5 py-2 text-[10px] text-zinc-500">
            Sem gatilho. Esta receita só executa manualmente ou quando chamada por uma rotina.
          </p>
        ) : (
          <div className="mt-2 space-y-2">
            {triggers.map((trigger, index) => (
              <TriggerEditor
                key={index}
                trigger={trigger}
                index={index}
                workspaceId={workspaceId}
                onChange={(value) =>
                  setTriggers(
                    triggers.map((item, position) =>
                      position === index ? value : item,
                    ),
                  )
                }
                onRemove={() =>
                  setTriggers(triggers.filter((_, position) => position !== index))
                }
              />
            ))}
          </div>
        )}
      </section>

      <section className="relative space-y-2 border-l-2 border-zinc-800 pl-4">
        <div className="flex items-center gap-2">
          <p className="text-[10px] font-semibold uppercase tracking-wide text-zinc-400">
            Ordem de execução
          </p>
          <CountPill value={flow.length} />
        </div>

        {flow.map((item, index) => (
          <div
            key={`${item.type}-${index}`}
            className={cn(
              "rounded-lg border p-2.5",
              item.type === "step" && "border-zinc-800 bg-zinc-950/40",
              item.type === "hitl" && "border-amber-500/30 bg-amber-500/[0.04]",
              item.type === "sink" && "border-emerald-500/30 bg-emerald-500/[0.04]",
            )}
          >
            <div className="flex items-center gap-1">
              <Badge
                variant={
                  item.type === "step" ? "default" : item.type === "hitl" ? "yellow" : "green"
                }
              >
                {item.type === "step" ? "Etapa" : item.type === "hitl" ? "Checkpoint" : "Saída"}
              </Badge>
              <input
                value={(item.name as string) ?? ""}
                onChange={(event) => patch(index, { name: event.target.value })}
                className={cn(inputClass, "flex-1")}
                placeholder="Nome exibido"
              />
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                aria-label="Mover etapa para cima"
                onClick={() => move(index, -1)}
                disabled={index === 0}
                title="Mover para cima"
              >
                <ChevronUp size={11} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                aria-label="Mover etapa para baixo"
                onClick={() => move(index, 1)}
                disabled={index === flow.length - 1}
                title="Mover para baixo"
              >
                <ChevronDown size={11} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 text-red-300"
                aria-label="Remover etapa"
                onClick={() => setFlow(flow.filter((_, position) => position !== index))}
                title="Remover"
              >
                <Trash2 size={11} />
              </Button>
            </div>

            {item.type === "step" ? (
              <StepEditor
                step={item}
                workspaceId={workspaceId}
                onChange={(value) => patch(index, value)}
              />
            ) : item.type === "hitl" ? (
              <HitlEditor
                checkpoint={item}
                onChange={(value) => patch(index, value)}
              />
            ) : (
              <SinkEditor
                sink={item}
                onChange={(value) => patch(index, value)}
              />
            )}
          </div>
        ))}

        {flow.length === 0 ? (
          <p className="rounded-lg border border-dashed border-zinc-700 px-3 py-6 text-center text-[10px] text-zinc-500">
            Adicione a primeira etapa para montar o fluxo.
          </p>
        ) : null}

        <div className="flex flex-wrap gap-1.5">
          <Button variant="outline" size="sm" onClick={addStep}>
            <IconLabel icon={<Plus size={11} />}>Etapa</IconLabel>
          </Button>
          <Button variant="outline" size="sm" onClick={addCheckpoint}>
            <IconLabel icon={<Plus size={11} />}>Checkpoint</IconLabel>
          </Button>
          <Button variant="outline" size="sm" onClick={addSink}>
            <IconLabel icon={<Plus size={11} />}>Saída</IconLabel>
          </Button>
        </div>
      </section>
    </div>
  );
}

function StepEditor({
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
      <div className="grid gap-1.5 sm:grid-cols-3">
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
            className="[&_input]:h-8 [&_input]:text-[10px]"
          />
        </LabeledField>
        <LabeledField label="Modelo">
          <ModelCombobox
            value={typeof step.model === "string" ? step.model : null}
            onChange={(value) => onChange({ model: value })}
          />
        </LabeledField>
      </div>
      <LabeledField label="Prompt da etapa">
        <SlashTextarea
          value={String(step.prompt ?? "")}
          onChange={(value) => onChange({ prompt: value })}
          workspaceId={workspaceId}
          rows={4}
          className="font-mono text-[10px]"
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

function HitlEditor({
  checkpoint,
  onChange,
}: {
  checkpoint: Dictionary;
  onChange: (value: Dictionary) => void;
}) {
  const type = String(checkpoint.interaction_type ?? "BINARY_GATE");
  const channels = Array.isArray(checkpoint.channels)
    ? (checkpoint.channels as string[])
    : [];
  return (
    <div className="mt-1.5 space-y-1.5">
      <div className="grid gap-1.5 sm:grid-cols-2">
        <LabeledField
          label="Tipo de interação"
          hint="Decide como o usuário responde ao checkpoint."
        >
          <select
            value={type}
            onChange={(event) => onChange({ interaction_type: event.target.value })}
            className={inputClass}
          >
            {HITL_TYPES.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </LabeledField>
        <LabeledField
          label="Tempo limite (min)"
          hint="Depois disso, a política de timeout é aplicada."
        >
          <input
            type="number"
            min={1}
            value={Number(checkpoint.timeout_minutes ?? 240)}
            onChange={(event) =>
              onChange({ timeout_minutes: Number(event.target.value) })
            }
            className={inputClass}
          />
        </LabeledField>
        <LabeledField label="Se o tempo acabar">
          <select
            value={String(checkpoint.on_timeout ?? "PAUSE_AND_ALERT")}
            onChange={(event) => onChange({ on_timeout: event.target.value })}
            className={inputClass}
          >
            {ON_TIMEOUT.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </LabeledField>
        <LabeledField label="Canais de aviso">
          <div className="flex flex-wrap gap-2 pt-1.5">
            {CHANNELS.map((channel) => (
              <label key={channel} className="flex items-center gap-1 text-[10px] text-zinc-400">
                <input
                  type="checkbox"
                  checked={channels.includes(channel)}
                  onChange={(event) =>
                    onChange({
                      channels: event.target.checked
                        ? [...channels, channel]
                        : channels.filter((item) => item !== channel),
                    })
                  }
                  className="accent-blue-500"
                />
                {channel}
              </label>
            ))}
          </div>
        </LabeledField>
      </div>
      {type === "MULTI_CHOICE" ? (
        <LabeledField
          label="Opções (separadas por vírgula)"
          hint="O usuário escolhe uma destas opções."
        >
          <input
            value={
              Array.isArray(checkpoint.options)
                ? (checkpoint.options as string[]).join(", ")
                : ""
            }
            onChange={(event) =>
              onChange({
                options: event.target.value
                  .split(",")
                  .map((value) => value.trim())
                  .filter(Boolean),
              })
            }
            className={inputClass}
            placeholder="aprovar, revisar, cancelar"
          />
        </LabeledField>
      ) : null}
      <LabeledField
        label="Mensagem do checkpoint"
        hint="Aceita variáveis como {{steps.passo-1.summary}}."
      >
        <textarea
          value={String(checkpoint.prompt_template ?? "")}
          onChange={(event) => onChange({ prompt_template: event.target.value })}
          rows={2}
          className="w-full rounded border border-zinc-800 bg-zinc-900/50 p-2 font-mono text-[10px] text-zinc-200 outline-none"
        />
      </LabeledField>
    </div>
  );
}

function SinkEditor({
  sink,
  onChange,
}: {
  sink: Dictionary;
  onChange: (value: Dictionary) => void;
}) {
  const config = (sink.config ?? {}) as Dictionary;
  return (
    <div className="mt-1.5 space-y-1.5">
      <div className="grid gap-1.5 sm:grid-cols-2">
        <LabeledField label="Tipo de saída">
          <select
            value={String(sink.sink_type ?? "DOC_COMPILER")}
            onChange={(event) => onChange({ sink_type: event.target.value })}
            className={inputClass}
          >
            {SINK_TYPES.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </LabeledField>
        <LabeledField
          label="Configuração (JSON)"
          hint="Caminhos e destinatários aceitam variáveis do fluxo."
        >
          <input
            value={JSON.stringify(config)}
            onChange={(event) => {
              try {
                onChange({ config: JSON.parse(event.target.value) });
              } catch {
                // Keep the last valid JSON while the user types.
              }
            }}
            className={cn(inputClass, "font-mono")}
            placeholder="{}"
          />
        </LabeledField>
      </div>
    </div>
  );
}

