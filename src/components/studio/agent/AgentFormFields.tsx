import { slugifyWorkspace } from "@/lib/folder";
import { StudioField } from "@/components/studio/StudioList";
import type { AgentFile, ModelInfo } from "@/lib/types";
import { cn } from "@/lib/utils";
import { AgentAvatarField } from "./AgentAvatarField";
import { AgentColorField } from "./AgentColorField";
import { inputClass } from "./agent-presets";

export function AgentFormFields({
  draft,
  onDraftChange,
  isNew,
  idTouched,
  setIdTouched,
  models,
}: {
  draft: AgentFile;
  onDraftChange: (next: AgentFile) => void;
  isNew: boolean;
  idTouched: boolean;
  setIdTouched: (touched: boolean) => void;
  models: ModelInfo[];
}) {
  return (
    <>
      <div className="grid gap-3 md:grid-cols-2">
        <StudioField label="Nome de exibição" hint="Usado pelo OpenDesk para identificar o agente; o OpenCode ignora este campo.">
          <input
            value={draft.name ?? ""}
            onChange={(event) => {
              const name = event.target.value;
              const next = { ...draft, name };
              if (isNew && !idTouched) {
                next.id = slugifyWorkspace(name);
              }
              onDraftChange(next);
            }}
            placeholder="Auditor de Qualidade"
            className={inputClass}
          />
        </StudioField>
        <StudioField
          label="ID / slug"
          hint={
            isNew
              ? "Gerado automaticamente do nome. Edite apenas se precisar."
              : "O ID não é renomeado por esta tela."
          }
        >
          <input
            value={draft.id}
            disabled={!isNew}
            onChange={(event) => {
              setIdTouched(true);
              onDraftChange({ ...draft, id: event.target.value });
            }}
            placeholder="auditor-de-qualidade"
            className={cn(inputClass, "font-mono disabled:opacity-60")}
          />
        </StudioField>
        <AgentAvatarField
          avatar={draft.avatar}
          onChange={(avatar) => onDraftChange({ ...draft, avatar })}
        />
        <AgentColorField
          color={draft.color}
          onChange={(color) => onDraftChange({ ...draft, color })}
        />
        <StudioField
          label="Política de roteador padrão"
          hint="Usada como default em rotinas que escolherem este agente."
        >
          <select
            value={draft.router_policy ?? ""}
            onChange={(event) =>
              onDraftChange({
                ...draft,
                router_policy: event.target.value || null,
              })
            }
            className={inputClass}
          >
            <option value="">herdar do workspace</option>
            <option value="COST_BENEFIT">Custo-benefício</option>
            <option value="CHEAPEST">Mais barato</option>
            <option value="BEST_QUALITY">Máxima qualidade</option>
            <option value="MANUAL">Manual</option>
            <option value="CUSTOM_RULES">Regras customizadas</option>
          </select>
        </StudioField>
        <StudioField label="Modo">
          <select
            value={draft.mode ?? "subagent"}
            onChange={(event) => onDraftChange({ ...draft, mode: event.target.value })}
            className={inputClass}
          >
            <option value="primary">primary</option>
            <option value="subagent">subagent</option>
            <option value="all">all</option>
          </select>
        </StudioField>
        <StudioField label="Modelo" hint="provider/model ou provider/model#variant.">
          <input
            list="opendesk-models"
            value={draft.model ?? ""}
            onChange={(event) => onDraftChange({ ...draft, model: event.target.value })}
            placeholder="opencode-go/glm-5.3"
            className={cn(inputClass, "font-mono text-[13px]")}
          />
          <datalist id="opendesk-models">
            {models.map((model) => {
              const provider = model.providerID ?? "";
              const id = model.modelID ?? model.id;
              return (
                <option
                  key={`${provider}/${id}`}
                  value={`${provider}/${id}`}
                />
              );
            })}
          </datalist>
        </StudioField>
        <StudioField label="Steps" hint="Máximo de passos do modelo; vazio usa o padrão.">
          <input
            type="number"
            min={1}
            value={draft.steps ?? ""}
            onChange={(event) =>
              onDraftChange({
                ...draft,
                steps: event.target.value ? Number(event.target.value) : null,
              })
            }
            className={inputClass}
          />
        </StudioField>
        <div className="flex items-end gap-4 pb-1">
          <label className="flex items-center gap-2 text-[13px] text-zinc-400">
            <input
              type="checkbox"
              checked={draft.hidden}
              onChange={(event) => onDraftChange({ ...draft, hidden: event.target.checked })}
              className="accent-blue-500"
            />
            hidden
          </label>
          <label className="flex items-center gap-2 text-[13px] text-zinc-400">
            <input
              type="checkbox"
              checked={draft.disabled}
              onChange={(event) => onDraftChange({ ...draft, disabled: event.target.checked })}
              className="accent-blue-500"
            />
            disabled
          </label>
        </div>
      </div>

      <StudioField label="Descrição">
        <input
          value={draft.description ?? ""}
          onChange={(event) => onDraftChange({ ...draft, description: event.target.value })}
          placeholder="Especialista em auditoria de qualidade"
          className={inputClass}
        />
      </StudioField>
    </>
  );
}
