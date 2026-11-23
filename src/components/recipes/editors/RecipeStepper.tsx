import { ChevronDown, ChevronUp, Plus, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { IconLabel } from "@/components/ui/icon-label";
import { FieldHint } from "@/components/ui/field-hint";
import { cn } from "@/lib/utils";
import { inputClass } from "../recipe-fields";
import { TriggerEditor } from "../recipe-trigger-editor";
import type { Dictionary, FlowEntry, FlowNodeType } from "../recipe-types";
import { commit, normalizeFlow, normalizeTriggers } from "./recipe-editor-model";
import { DagNodeEditor } from "./DagNodeEditor";
import { HitlEditor } from "./HitlEditor";
import { SinkEditor } from "./SinkEditor";
import { StepEditor } from "./StepEditor";

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
        channels: ["DESKTOP_UI", "PUSH"],
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

  function addComplex(type: FlowNodeType) {
    const count = flow.filter((item) => item.type === type).length + 1;
    const base: Record<FlowNodeType, Dictionary> = {
      step: {},
      hitl: {},
      sink: {},
      branch: { id: `branch-${count}`, when: "true", then: [], else: [] },
      parallel: { id: `parallel-${count}`, join: "all", branches: [[], []] },
      map: {
        id: `map-${count}`,
        items: "{{steps.lista.structured.items}}",
        item_var: "item",
        max_concurrency: 3,
        flow: [],
      },
      recipe_call: { id: `call-${count}`, recipe: "outra-receita", with: {} },
      loop: { id: `loop-${count}`, max_iterations: 3, flow: [] },
      switch: { id: `switch-${count}`, cases: [], default: [] },
    };
    const labels: Record<FlowNodeType, string> = {
      step: "Etapa",
      hitl: "Checkpoint",
      sink: "Saída",
      branch: "Condição",
      parallel: "Paralelo",
      map: "Map",
      recipe_call: "Sub-receita",
      loop: "Loop",
      switch: "Switch",
    };
    setFlow([...flow, { ...base[type], type, name: labels[type] }]);
  }

  return (
    <div className="space-y-3">
      <label className="block">
        <span className="mb-1 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
          Nome da receita
        </span>
        <input
          value={String(recipe.name ?? "")}
          onChange={(event) => onChange({ ...recipe, name: event.target.value })}
          placeholder="Nome exibido nas listas e execuções"
          className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 text-[13.5px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
        />
      </label>
      <section className="rounded-lg border border-blue-500/30 bg-blue-500/[0.04] p-2.5">
        <div className="flex flex-wrap items-center gap-2">
          <p className="text-[12px] font-semibold uppercase tracking-wide text-blue-200">
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
          <p className="mt-2 rounded border border-dashed border-zinc-700 px-2.5 py-2 text-[12px] text-zinc-500">
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

      <section className="relative space-y-2 border-l border-zinc-800 pl-4">
        <div className="flex items-center gap-2">
          <p className="text-[12px] font-semibold uppercase tracking-wide text-zinc-400">
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
                  item.type === "step"
                    ? "default"
                    : item.type === "hitl"
                      ? "yellow"
                      : item.type === "sink"
                        ? "green"
                        : "purple"
                }
              >
                {nodeLabel(item.type)}
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
            ) : item.type === "sink" ? (
              <SinkEditor
                sink={item}
                onChange={(value) => patch(index, value)}
              />
            ) : (
              <DagNodeEditor
                node={item}
                onChange={(value) => patch(index, value)}
              />
            )}
          </div>
        ))}

        {flow.length === 0 ? (
          <p className="rounded-lg border border-dashed border-zinc-700 px-3 py-6 text-center text-[12px] text-zinc-500">
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
          <Button variant="outline" size="sm" onClick={() => addComplex("branch")}>
            <IconLabel icon={<Plus size={11} />}>Condição</IconLabel>
          </Button>
          <Button variant="outline" size="sm" onClick={() => addComplex("parallel")}>
            <IconLabel icon={<Plus size={11} />}>Paralelo</IconLabel>
          </Button>
          <Button variant="outline" size="sm" onClick={() => addComplex("map")}>
            <IconLabel icon={<Plus size={11} />}>Map</IconLabel>
          </Button>
          <Button variant="outline" size="sm" onClick={() => addComplex("recipe_call")}>
            <IconLabel icon={<Plus size={11} />}>Sub-receita</IconLabel>
          </Button>
          <Button variant="outline" size="sm" onClick={() => addComplex("loop")}>
            <IconLabel icon={<Plus size={11} />}>Loop</IconLabel>
          </Button>
          <Button variant="outline" size="sm" onClick={() => addComplex("switch")}>
            <IconLabel icon={<Plus size={11} />}>Switch</IconLabel>
          </Button>
        </div>
      </section>
    </div>
  );
}

function nodeLabel(type: FlowNodeType): string {
  switch (type) {
    case "step":
      return "Etapa";
    case "hitl":
      return "Checkpoint";
    case "sink":
      return "Saída";
    case "branch":
      return "Condição";
    case "parallel":
      return "Paralelo";
    case "map":
      return "Map";
    case "recipe_call":
      return "Sub-receita";
    case "loop":
      return "Loop";
    case "switch":
      return "Switch";
  }
}
