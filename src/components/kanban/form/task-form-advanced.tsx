import type { Dispatch, SetStateAction } from "react";
import { Globe } from "lucide-react";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { HARNESS_OPTIONS } from "@/lib/harnesses";
import { CollapsibleSection } from "@/components/ui/collapsible-section";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { Switch } from "@/components/ui/switch";
import { PRIORITY_LABEL } from "@/lib/task-ui";
import { fieldInputClass as inputClass, fieldLabelClass as labelClass } from "@/lib/ui-classes";

export interface TaskFormAdvancedProps {
  workspaceId: string | null;
  executionMode: string;
  setExecutionMode: (value: string) => void;
  priority: number;
  setPriority: (value: number) => void;
  agent: string;
  setAgent: (value: string) => void;
  model: string | null;
  setModel: Dispatch<SetStateAction<string | null>>;
  harness: string;
  setHarness: (value: string) => void;
  effort: string;
  setEffort: (value: string) => void;
  careLevel: "quick" | "standard" | "careful";
  setCareLevel: Dispatch<SetStateAction<"quick" | "standard" | "careful">>;
  tags: string;
  setTags: (value: string) => void;
  needsBrowser: boolean;
  setNeedsBrowser: Dispatch<SetStateAction<boolean>>;
  browseAllowlist: string;
  setBrowseAllowlist: (value: string) => void;
  email: string;
  setEmail: (value: string) => void;
  watchChannels: string[];
  setWatchChannels: Dispatch<SetStateAction<string[]>>;
  exportToGdocs: boolean;
  setExportToGdocs: Dispatch<SetStateAction<boolean>>;
}

export function TaskFormAdvanced({
  workspaceId,
  executionMode,
  setExecutionMode,
  priority,
  setPriority,
  agent,
  setAgent,
  model,
  setModel,
  harness,
  setHarness,
  effort,
  setEffort,
  careLevel,
  setCareLevel,
  tags,
  setTags,
  needsBrowser,
  setNeedsBrowser,
  browseAllowlist,
  setBrowseAllowlist,
  email,
  setEmail,
  watchChannels,
  setWatchChannels,
  exportToGdocs,
  setExportToGdocs,
}: TaskFormAdvancedProps) {
  const advancedSummary = [
    model ? `modelo: ${model}` : "roteador automático",
    `prioridade ${priority}`,
    agent ? `@${agent}` : null,
    careLevel !== "standard" ? `cuidado: ${careLevel}` : null,
    tags.trim() ? "tags" : null,
    email.trim() ? "e-mail" : null,
    watchChannels.length > 0
      ? `avisos: ${watchChannels.join(", ")}`
      : null,
    exportToGdocs ? "Google Docs" : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <CollapsibleSection title="Mais opções" summary={advancedSummary}>
      <div className="grid gap-3 md:grid-cols-2">
        <label>
          <span className={labelClass}>Modo</span>
          <select
            value={executionMode}
            onChange={(event) => setExecutionMode(event.target.value)}
            className={inputClass}
          >
            <option value="AUTO_DSH">AUTO_DSH — roteador decide</option>
            <option value="PLAN_ONLY">PLAN_ONLY — só plano</option>
            <option value="PLAN_THEN_BUILD">PLAN_THEN_BUILD — aprovar plano</option>
            <option value="BUILD_DIRECT">BUILD_DIRECT — executar direto</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Prioridade</span>
          <select
            value={priority}
            onChange={(event) => setPriority(Number(event.target.value))}
            className={inputClass}
          >
            {[4, 3, 2, 1].map((value) => (
              <option key={value} value={value}>
                {value} — {PRIORITY_LABEL[value]}
              </option>
            ))}
          </select>
        </label>
        <div>
          <span className={labelClass}>Agente (opcional)</span>
          <AgentCombobox
            value={agent}
            onChange={(value) => setAgent(value)}
            workspaceId={workspaceId}
            placeholder="automático (router)"
          />
        </div>
        <div>
          <span className={labelClass}>Modelo (opcional)</span>
          <ModelCombobox
            value={model}
            onChange={setModel}
            placeholder="digite para filtrar (ex.: deepseek, glm, opencode-go/…)"
          />
        </div>
        <label>
          <span className={labelClass}>Harness (opcional)</span>
          <select
            value={harness}
            onChange={(event) => setHarness(event.target.value)}
            className={inputClass}
          >
            <option value="">automático (padrão do workspace)</option>
            {HARNESS_OPTIONS.map((option) => (
              <option key={option.id} value={option.id}>
                {option.label} — {option.description}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span className={labelClass}>Effort</span>
          <select
            value={effort}
            onChange={(event) => setEffort(event.target.value)}
            className={inputClass}
          >
            <option value="low">low</option>
            <option value="medium">medium</option>
            <option value="high">high</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Cuidado</span>
          <select
            value={careLevel}
            onChange={(event) =>
              setCareLevel(event.target.value as "quick" | "standard" | "careful")
            }
            className={inputClass}
          >
            <option value="quick">rápido (menos passos)</option>
            <option value="standard">padrão (checklist curto)</option>
            <option value="careful">cuidadoso (revisão completa)</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Tags (vírgula)</span>
          <input
            value={tags}
            onChange={(event) => setTags(event.target.value)}
            placeholder="roteiro, code, auditoria"
            className={inputClass}
          />
        </label>
        <div className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2 sm:col-span-2">
          <div className="min-w-0">
            <p className="flex items-center gap-1.5 text-[13px] text-zinc-300">
              <Globe size={12} className="text-sky-300" />
              Precisa navegar na internet
            </p>
            <p className="mt-0.5 text-[11.5px] leading-snug text-zinc-500">
              Ativa o OpenCode Browse automaticamente e inclui a skill de navegação
              no prompt. Login, 2FA e captcha são entregues a você.
            </p>
          </div>
          <Switch
            checked={needsBrowser}
            onCheckedChange={setNeedsBrowser}
            label="Precisa navegar na internet"
          />
        </div>
        {needsBrowser ? (
          <label className="sm:col-span-2">
            <span className={labelClass}>Domínios permitidos (opcional)</span>
            <input
              value={browseAllowlist}
              onChange={(event) => setBrowseAllowlist(event.target.value)}
              placeholder="example.com, docs.example.com"
              className={inputClass}
            />
            <span className="mt-1 block text-[11.5px] leading-snug text-zinc-500">
              Limita a sessão do Browse a estes domínios. Tarefas marcadas com
              browse-high-risk, browse-login, browse-payment, browse-banking ou
              browse-2fa exigem allowlist.
            </span>
          </label>
        ) : null}
        <label>
          <span className={labelClass}>E-mail de notificação</span>
          <input
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="ops@empresa.com"
            className={inputClass}
          />
        </label>
        <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2 sm:col-span-2">
          <p className="text-[13px] text-zinc-300">Avisar quando concluir</p>
          <p className="mt-0.5 text-[11.5px] leading-snug text-zinc-500">
            Sem escolha aqui, a tarefa não avisa ninguém. Ao marcar um canal, o
            resultado, o status e o artefato principal são enviados ao terminar
            cada execução, inclusive em falhas.
          </p>
          <div className="mt-2 flex flex-wrap gap-4">
            {(["telegram", "signal"] as const).map((channel) => (
              <label
                key={channel}
                className="flex items-center gap-2 text-[12.5px] text-zinc-400"
              >
                <input
                  type="checkbox"
                  checked={watchChannels.includes(channel)}
                  onChange={(event) =>
                    setWatchChannels((current) =>
                      event.target.checked
                        ? [...current.filter((item) => item !== channel), channel]
                        : current.filter((item) => item !== channel),
                    )
                  }
                  className="accent-blue-500"
                />
                {channel === "telegram" ? "Telegram" : "Signal"}
              </label>
            ))}
          </div>
        </div>
        <label className="flex items-end gap-2 pb-2 text-[13px] text-zinc-400">
          <input
            type="checkbox"
            checked={exportToGdocs}
            onChange={(event) => setExportToGdocs(event.target.checked)}
            className="accent-blue-500"
          />
          exportar para Google Docs
        </label>
      </div>
    </CollapsibleSection>
  );
}
