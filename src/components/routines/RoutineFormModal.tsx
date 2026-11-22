import {
  ChevronDown,
  Loader2,
  Plus,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { FieldHint } from "@/components/ui/field-hint";
import { IconLabel } from "@/components/ui/icon-label";
import { PromptAttachments } from "@/components/ui/prompt-attachments";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import type { Schedule } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import { shouldCloseOnBlur } from "@/lib/combobox";
import { TriggerSection } from "@/components/routines/TriggerSection";
import { useRoutineForm } from "@/components/routines/use-routine-form";

export type { RecipeOption } from "@/components/routines/use-routine-form";

export function CreateScheduleModal({
  open,
  workspaceId,
  schedule,
  onClose,
  onCreated,
}: {
  open: boolean;
  workspaceId: string;
  schedule: Schedule | null;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  const {
    workspace,
    name,
    setName,
    description,
    setDescription,
    triggerType,
    setTriggerType,
    triggerConfig,
    setTriggerConfig,
    targetKind,
    setTargetKind,
    setRecipeId,
    recipeQuery,
    setRecipeQuery,
    recipeOpen,
    setRecipeOpen,
    whenOpen,
    setWhenOpen,
    prompt,
    attachments,
    setAttachments,
    agent,
    setAgent,
    agentOpen,
    setAgentOpen,
    policy,
    setPolicy,
    setPolicyTouched,
    template,
    setTemplate,
    recipients,
    setRecipients,
    signalEnabled,
    setSignalEnabled,
    signalRecipients,
    setSignalRecipients,
    telegramEnabled,
    setTelegramEnabled,
    telegramRecipients,
    setTelegramRecipients,
    onlyOnChange,
    setOnlyOnChange,
    timezone,
    setTimezone,
    sinks,
    selectedSinks,
    setSelectedSinks,
    recurrence,
    setRecurrence,
    preview,
    slash,
    setSlash,
    promptRef,
    saving,
    error,
    chooseAgent,
    updatePrompt,
    insertSlash,
    slashItems,
    cancel,
    submit,
    filteredAgents,
    selectedRecipe,
    filteredRecipes,
  } = useRoutineForm({ open, workspaceId, schedule, onClose, onCreated });

  return (
    <Modal
      open={open}
      onClose={cancel}
      title={schedule ? "Editar rotina" : "Nova rotina"}
      description="A rotina pertence ao workspace ativo. Mude de workspace na barra superior para gerenciar as rotinas de outro workspace. Toda rotina nova nasce desligada; ative quando revisar."
      size="lg"
    >
      <div className="space-y-3">
        <div className="grid gap-2 md:grid-cols-2">
          <label>
            <span className={labelClass}>Workspace</span>
            <input value={workspace?.name ?? workspaceId} disabled className={inputClass} />
          </label>
          <label>
            <span className={labelClass}>Nome da rotina</span>
            <input value={name} onChange={(event) => setName(event.target.value)} className={inputClass} />
          </label>
        </div>

        <TriggerSection
          triggerType={triggerType}
          setTriggerType={setTriggerType}
          triggerConfig={triggerConfig}
          setTriggerConfig={setTriggerConfig}
          whenOpen={whenOpen}
          setWhenOpen={setWhenOpen}
          recurrence={recurrence}
          setRecurrence={setRecurrence}
          preview={preview}
          timezone={timezone}
          setTimezone={setTimezone}
        />

        <div className="rounded-xl border border-zinc-800/70 bg-zinc-950/30 p-3">
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-[12px] font-medium uppercase tracking-wide text-zinc-500">
              O que executar
            </p>
            <div className="ml-auto flex items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-950/60 p-0.5">
              <button
                type="button"
                onClick={() => setTargetKind("PROMPT")}
                className={cn(
                  "rounded-md px-2 py-1 text-[12px]",
                  targetKind === "PROMPT"
                    ? "workspace-tab-active"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                Prompt
              </button>
              <button
                type="button"
                onClick={() => setTargetKind("RECIPE")}
                className={cn(
                  "rounded-md px-2 py-1 text-[12px]",
                  targetKind === "RECIPE"
                    ? "workspace-tab-active"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                Receita
              </button>
            </div>
          </div>

          {targetKind === "RECIPE" ? (
            <div className="relative mt-2">
              <span className={labelClass}>Receita</span>
              <IconLabel
                icon={<ChevronDown size={12} className="text-zinc-500" />}
                className="pointer-events-none absolute right-2 top-[30px] text-zinc-500"
              />
              <input
                value={recipeOpen ? recipeQuery : (selectedRecipe?.name ?? "")}
                onChange={(event) => {
                  setRecipeQuery(event.target.value);
                  setRecipeOpen(true);
                }}
                onFocus={() => {
                  setRecipeQuery("");
                  setRecipeOpen(true);
                }}
                onBlur={(event) => {
                  if (shouldCloseOnBlur(event)) setRecipeOpen(false);
                }}
                placeholder="Digite para buscar a receita…"
                className={inputClass}
              />
              <FieldHint
                hint="A rotina executa o fluxo da receita, com os checkpoints e saídas definidos nela."
                className="absolute right-7 top-[32px]"
              />
              {recipeOpen ? (
                <div className="absolute z-30 mt-1 max-h-56 w-full overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl">
                  {filteredRecipes.length === 0 ? (
                    <p className="px-2 py-1 text-[12px] text-zinc-500">
                      Nenhuma receita encontrada.
                    </p>
                  ) : (
                    filteredRecipes.map((item) => (
                      <button
                        key={item.id}
                        type="button"
                        className="block w-full rounded px-2 py-1.5 text-left text-[12px] hover:bg-zinc-800"
                        onMouseDown={(event) => {
                          event.preventDefault();
                          setRecipeId(item.id);
                          setRecipeQuery("");
                          setRecipeOpen(false);
                        }}
                      >
                        <span className="text-zinc-200">{item.name}</span>
                        <span className="ml-2 font-mono text-[11.5px] text-zinc-500">
                          {item.id}
                        </span>
                      </button>
                    ))
                  )}
                </div>
              ) : null}
            </div>
          ) : (
            <>
              <div className="mt-2 grid gap-2 md:grid-cols-2">
                <label className="relative">
                  <span className={labelClass}>Agente (opcional)</span>
                  <input
                    value={agent}
                    onChange={(event) => {
                      setAgent(event.target.value);
                      setAgentOpen(true);
                    }}
                    onFocus={() => setAgentOpen(true)}
                    onBlur={(event) => {
                      if (shouldCloseOnBlur(event)) setAgentOpen(false);
                    }}
                    placeholder="automático (router)"
                    className={inputClass}
                  />
                  {agentOpen ? (
                    <div className="absolute z-30 mt-1 max-h-56 w-full overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl">
                      <button
                        type="button"
                        className="block w-full rounded px-2 py-1 text-left text-[12px] text-zinc-400 hover:bg-zinc-800"
                        onMouseDown={(event) => {
                          event.preventDefault();
                          chooseAgent(null);
                        }}
                      >
                        automático (router)
                      </button>
                      {filteredAgents.slice(0, 60).map((item) => (
                        <button
                          key={item.id}
                          type="button"
                          className="block w-full rounded px-2 py-1 text-left text-[12px] hover:bg-zinc-800"
                          onMouseDown={(event) => {
                            event.preventDefault();
                            chooseAgent(item);
                          }}
                        >
                          <span className="font-mono text-zinc-200">@{item.id}</span>
                          <span className="ml-1 text-zinc-500">{item.name}</span>
                          {item.router_policy ? (
                            <span className="ml-1 text-zinc-500">
                              {item.router_policy}
                            </span>
                          ) : null}
                        </button>
                      ))}
                      {filteredAgents.length === 0 ? (
                        <p className="px-2 py-1 text-[12px] text-zinc-500">
                          Nenhum agente encontrado.
                        </p>
                      ) : null}
                    </div>
                  ) : null}
                </label>
                <label>
                  <span className={labelClass}>Política do roteador</span>
                  <select
                    value={policy}
                    onChange={(event) => {
                      setPolicy(event.target.value);
                      setPolicyTouched(true);
                    }}
                    className={inputClass}
                  >
                    {Object.entries(ROUTER_POLICY_LABELS).map(([value, label]) => (
                      <option key={value} value={value}>
                        {label}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span className={labelClass}>Template da pasta de saída</span>
                  <input
                    value={template}
                    onChange={(event) => setTemplate(event.target.value)}
                    className={cn(inputClass, "font-mono text-[12px]")}
                  />
                </label>
              </div>

              <label className="relative mt-2 block">
                <span className={labelClass}>Comando ou prompt</span>
                <textarea
                  ref={promptRef}
                  value={prompt}
                  onChange={(event) =>
                    updatePrompt(
                      event.target.value,
                      event.target.selectionStart ?? 0,
                    )
                  }
                  onKeyDown={(event) => {
                    if (event.key === "Escape") setSlash(null);
                  }}
                  rows={5}
                  placeholder="Digite / para comandos e skills, ou descreva a tarefa"
                  className={cn(
                    inputClass,
                    "h-auto resize-y py-2 font-mono text-[13px]",
                  )}
                />
                {slash ? (
                  <div className="absolute z-30 mt-1 max-h-64 w-full overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl">
                    {slashItems.length === 0 ? (
                      <p className="px-2 py-1 text-[12px] text-zinc-500">
                        Nenhum comando/skill encontrado.
                      </p>
                    ) : (
                      slashItems.map((item) => (
                        <button
                          key={`${item.label}:${item.detail}`}
                          type="button"
                          className="block w-full rounded px-2 py-1 text-left hover:bg-zinc-800"
                          onMouseDown={(event) => {
                            event.preventDefault();
                            insertSlash(item.value);
                          }}
                        >
                          <span className="font-mono text-[12px] text-zinc-200">
                            {item.label}
                          </span>
                          <span className="ml-2 text-[11.5px] text-zinc-500">
                            {item.detail}
                          </span>
                        </button>
                      ))
                    )}
                  </div>
                ) : null}
              </label>
              {targetKind === "PROMPT" ? (
                <PromptAttachments
                  workspaceId={workspaceId}
                  value={attachments}
                  onChange={setAttachments}
                />
              ) : null}
            </>
          )}
        </div>
        <label>
          <span className={labelClass}>Descrição</span>
          <input value={description} onChange={(event) => setDescription(event.target.value)} className={inputClass} />
        </label>

        <div className="rounded-xl border border-zinc-800/70 bg-zinc-950/30 p-3">
          <p className="text-[12px] font-medium uppercase tracking-wide text-zinc-500">Notificações</p>
          <div className="mt-2 grid gap-2 md:grid-cols-2">
            <label>
              <span className={labelClass}>E-mail</span>
              <input value={recipients} onChange={(event) => setRecipients(event.target.value)} placeholder="a@empresa.com, b@empresa.com" className={inputClass} />
            </label>
            <label>
              <span className={labelClass}>Signal (números, opcional)</span>
              <input value={signalRecipients} onChange={(event) => setSignalRecipients(event.target.value)} placeholder="vazio usa a allowlist do Signal" className={inputClass} />
            </label>
            <label>
              <span className={labelClass}>Telegram (chats, opcional)</span>
              <input value={telegramRecipients} onChange={(event) => setTelegramRecipients(event.target.value)} placeholder="vazio usa a allowlist do Telegram" className={inputClass} />
            </label>
          </div>
          <div className="mt-2 flex flex-wrap gap-4 text-[13px] text-zinc-400">
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={signalEnabled} onChange={(event) => setSignalEnabled(event.target.checked)} className="accent-blue-500" />
              notificar por Signal
            </label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={telegramEnabled} onChange={(event) => setTelegramEnabled(event.target.checked)} className="accent-blue-500" />
              notificar por Telegram
            </label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={onlyOnChange} onChange={(event) => setOnlyOnChange(event.target.checked)} className="accent-blue-500" />
              notificar somente se houver novidade
            </label>
          </div>
        </div>

        {sinks.length > 0 ? (
          <div className="rounded-xl border border-zinc-800/70 bg-zinc-950/30 p-3">
            <p className="text-[12px] font-medium uppercase tracking-wide text-zinc-500">
              Entregas automáticas
            </p>
            <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1">
              {sinks.map((sink) => (
                <label key={sink.id} className="flex items-center gap-1.5 text-[13px] text-zinc-400">
                  <input
                    type="checkbox"
                    checked={selectedSinks.has(sink.id)}
                    onChange={(event) =>
                      setSelectedSinks((current) => {
                        const next = new Set(current);
                        if (event.target.checked) next.add(sink.id);
                        else next.delete(sink.id);
                        return next;
                      })
                    }
                    className="accent-blue-500"
                  />
                  {sink.name}
                </label>
              ))}
            </div>
          </div>
        ) : null}

        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">{error}</p>
        ) : null}
      </div>
      <div className="mt-4 flex justify-end gap-2 border-t border-zinc-800/70 pt-4">
        <Button variant="ghost" size="sm" onClick={cancel}>Cancelar</Button>
        <Button size="sm" onClick={() => void submit()} disabled={saving}>
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          {schedule ? "Salvar rotina" : "Criar rotina"}
        </Button>
      </div>
    </Modal>
  );
}