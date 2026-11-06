import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CalendarClock,
  Clock3,
  Loader2,
  Mail,
  Pause,
  Play,
  Plus,
  RefreshCw,
  Timer,
  Trash2,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import type { Schedule } from "@/lib/types";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";

export function RoutinesPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const lastScheduleEvent = useAppStore((state) => state.lastScheduleEvent);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );

  const [schedules, setSchedules] = useState<Schedule[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);

  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const query = useMemo(
    () => (activeWorkspaceId ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}` : ""),
    [activeWorkspaceId],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setSchedules([]);
      return;
    }
    setLoading(true);
    try {
      const items = await apiGet<Schedule[]>(`/api/schedules${query}`);
      setSchedules(items);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    void load();
  }, [load, lastScheduleEvent?.schedule_id]);

  async function toggle(schedule: Schedule) {
    try {
      await apiPost(`/api/schedules/${schedule.id}/active`, {
        active: !schedule.is_active,
      });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function runNow(schedule: Schedule) {
    try {
      await apiPost(`/api/schedules/${schedule.id}/run`, {});
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function remove(schedule: Schedule) {
    if (!window.confirm(`Excluir a rotina ${schedule.name}?`)) return;
    try {
      await apiDelete(`/api/schedules/${schedule.id}`);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader eyebrow="automação recorrente" title="Rotinas (Cron)" />
        <EmptyState
          icon={CalendarClock}
          title="Nenhum workspace ativo"
          description="Selecione um workspace para gerenciar rotinas."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="automação recorrente"
        title="Rotinas (Cron)"
        description={`${workspace?.name ?? "Workspace"} · agendamento com pasta determinística, fila, e-mail e histórico.`}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void load()}>
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
            <Button size="sm" onClick={() => setCreateOpen(true)}>
              <Plus size={15} />
              Nova rotina
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="grid gap-3 md:grid-cols-3">
        {[
          {
            icon: Clock3,
            title: "Agendamento",
            description: "Expressão cron, timezone, próxima execução e recuperação de slots.",
          },
          {
            icon: Play,
            title: "Execução determinística",
            description: "output/{YYYY}-{MM}/{schedule_slug} criado antes de enfileirar.",
          },
          {
            icon: Mail,
            title: "Entrega",
            description: "Resumo e anexos por SMTP, com idempotência em integration_deliveries.",
          },
        ].map((item) => (
          <Card key={item.title}>
            <CardContent className="p-4">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/60 text-zinc-500">
                <item.icon size={15} strokeWidth={1.7} />
              </div>
              <p className="mt-3 text-[12px] font-medium text-zinc-200">{item.title}</p>
              <p className="mt-1 text-[11px] leading-relaxed text-zinc-500">{item.description}</p>
            </CardContent>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Timer size={15} className="text-zinc-500" />
            Rotinas do workspace
          </CardTitle>
          <Badge variant="default">{schedules.length} agendadas</Badge>
        </CardHeader>
        <CardContent>
          {schedules.length === 0 ? (
            <EmptyState
              icon={CalendarClock}
              title="Nenhuma rotina configurada"
              description="Crie uma rotina com expressão cron, comando/prompt e destinatários. O motor roda a cada 15 s e usa o slot agendado como chave de idempotência."
              action={
                <Button size="sm" onClick={() => setCreateOpen(true)}>
                  <Plus size={14} />
                  Criar rotina
                </Button>
              }
            />
          ) : (
            <div className="space-y-2">
              {schedules.map((schedule) => (
                <div
                  key={schedule.id}
                  className={cn(
                    "rounded-xl border px-4 py-3",
                    schedule.is_active
                      ? "border-zinc-800/70 bg-zinc-950/40"
                      : "border-zinc-800/50 bg-zinc-950/20 opacity-70",
                  )}
                >
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-[12px] font-medium text-zinc-100">{schedule.name}</p>
                        <Badge variant={schedule.is_active ? "green" : "default"}>
                          {schedule.is_active ? "ativa" : "pausada"}
                        </Badge>
                        <Badge variant="blue">{schedule.cron_expression}</Badge>
                        <Badge variant="default">{schedule.timezone}</Badge>
                        {schedule.agent_name ? (
                          <Badge variant="purple">@{schedule.agent_name}</Badge>
                        ) : null}
                        <Badge variant="default">
                          {ROUTER_POLICY_LABELS[schedule.router_policy] ?? schedule.router_policy}
                        </Badge>
                      </div>
                      <p className="mt-1 truncate font-mono text-[10px] text-zinc-500">
                        {schedule.command_or_prompt}
                      </p>
                      <p className="mt-1 font-mono text-[9px] text-zinc-600">
                        {schedule.output_folder_template}
                      </p>
                      <div className="mt-2 flex flex-wrap items-center gap-3 font-mono text-[9px] text-zinc-600">
                        <span>
                          próxima:{" "}
                          {schedule.next_run_at
                            ? new Date(schedule.next_run_at).toLocaleString("pt-BR")
                            : "—"}
                        </span>
                        <span>
                          última:{" "}
                          {schedule.last_run_at
                            ? `${relativeTime(schedule.last_run_at)} (${schedule.last_run_status ?? "—"})`
                            : "—"}
                        </span>
                        {schedule.notify_email_recipients ? (
                          <span className="flex items-center gap-1 text-zinc-500">
                            <Mail size={10} />
                            {schedule.notify_email_recipients}
                          </span>
                        ) : null}
                      </div>
                    </div>
                    <div className="flex shrink-0 flex-wrap gap-2">
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        onClick={() => void runNow(schedule)}
                      >
                        <Play size={12} />
                        Executar agora
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        onClick={() => void toggle(schedule)}
                      >
                        {schedule.is_active ? <Pause size={12} /> : <Play size={12} />}
                        {schedule.is_active ? "Pausar" : "Retomar"}
                      </Button>
                      <Button
                        variant="destructive"
                        size="sm"
                        className="h-7"
                        onClick={() => void remove(schedule)}
                      >
                        <Trash2 size={12} />
                      </Button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <CreateScheduleModal
        open={createOpen}
        workspaceId={activeWorkspaceId}
        onClose={() => setCreateOpen(false)}
        onCreated={async () => {
          setCreateOpen(false);
          await load();
        }}
      />
    </PageBody>
  );
}

function CreateScheduleModal({
  open,
  workspaceId,
  onClose,
  onCreated,
}: {
  open: boolean;
  workspaceId: string;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [cron, setCron] = useState("0 8 * * 1-5");
  const [timezone, setTimezone] = useState("America/Sao_Paulo");
  const [prompt, setPrompt] = useState("");
  const [agent, setAgent] = useState("");
  const [policy, setPolicy] = useState("COST_BENEFIT");
  const [template, setTemplate] = useState("output/{YYYY}-{MM}/{schedule_slug}");
  const [recipients, setRecipients] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    if (!name.trim() || !prompt.trim()) {
      setError("Informe nome e comando/prompt.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await apiPost("/api/schedules", {
        workspace_id: workspaceId,
        name: name.trim(),
        description: description.trim() || null,
        cron_expression: cron.trim(),
        timezone: timezone.trim(),
        agent_name: agent.trim() || null,
        command_or_prompt: prompt.trim(),
        router_policy: policy,
        output_folder_template: template.trim(),
        notify_email_recipients: recipients.trim() || null,
        sync_to_google_docs: false,
        sync_to_google_calendar: false,
        is_active: true,
      });
      setName("");
      setPrompt("");
      setRecipients("");
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
      title="Nova rotina"
      description="A rotina cria a pasta determinística, aplica o roteador e enfileira no horário agendado. Use /comando para expandir um comando do Studio."
      className="max-w-3xl"
    >
      <div className="grid gap-3 md:grid-cols-2">
        <label>
          <span className={labelClass}>Nome</span>
          <input value={name} onChange={(e) => setName(e.target.value)} className={inputClass} />
        </label>
        <label>
          <span className={labelClass}>Expressão cron</span>
          <input
            value={cron}
            onChange={(e) => setCron(e.target.value)}
            placeholder="0 8 * * 1-5"
            className={cn(inputClass, "font-mono")}
          />
        </label>
        <label>
          <span className={labelClass}>Timezone</span>
          <input
            value={timezone}
            onChange={(e) => setTimezone(e.target.value)}
            placeholder="America/Sao_Paulo"
            className={cn(inputClass, "font-mono")}
          />
        </label>
        <label>
          <span className={labelClass}>Agente (opcional)</span>
          <input
            value={agent}
            onChange={(e) => setAgent(e.target.value)}
            placeholder="build, plan, explorar…"
            className={inputClass}
          />
        </label>
        <label className="md:col-span-2">
          <span className={labelClass}>Comando ou prompt</span>
          <textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            rows={4}
            placeholder="/relatorio-diario ou descreva a tarefa completa"
            className={cn(inputClass, "h-auto resize-y py-2")}
          />
        </label>
        <label>
          <span className={labelClass}>Política do roteador</span>
          <select value={policy} onChange={(e) => setPolicy(e.target.value)} className={inputClass}>
            {Object.entries(ROUTER_POLICY_LABELS).map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span className={labelClass}>Destinatários de e-mail</span>
          <input
            value={recipients}
            onChange={(e) => setRecipients(e.target.value)}
            placeholder="a@empresa.com, b@empresa.com"
            className={inputClass}
          />
        </label>
        <label className="md:col-span-2">
          <span className={labelClass}>Template da pasta de saída</span>
          <input
            value={template}
            onChange={(e) => setTemplate(e.target.value)}
            className={cn(inputClass, "font-mono text-[11px]")}
          />
        </label>
        <label className="md:col-span-2">
          <span className={labelClass}>Descrição</span>
          <input
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className={inputClass}
          />
        </label>
      </div>
      {error ? (
        <p className="mt-3 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}
      <div className="mt-4 flex justify-end gap-2 border-t border-zinc-800/70 pt-4">
        <Button variant="ghost" size="sm" onClick={onClose}>
          Cancelar
        </Button>
        <Button size="sm" onClick={() => void submit()} disabled={saving}>
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          Criar rotina
        </Button>
      </div>
    </Modal>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60";
const labelClass =
  "mb-1.5 block text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500";
