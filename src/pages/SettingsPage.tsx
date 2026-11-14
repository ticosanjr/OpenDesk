import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  Archive,
  Bell,
  Check,
  Cpu,
  Download,
  ExternalLink,
  FolderOpen,
  FolderPlus,
  HardDrive,
  Info,
  ListChecks,
  Loader2,
  Save,
  Settings,
  ShieldCheck,
  Wrench,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { EngineCard } from "@/components/settings/EngineCard";
import { UpdatesCard } from "@/components/settings/UpdatesCard";
import { AboutCard } from "@/components/settings/AboutCard";
import { NotificationCard } from "@/components/settings/NotificationCard";
import { SectionNav } from "@/components/ui/section-nav";
import {
  Field,
  FormError,
  Input,
  Select,
} from "@/components/ui/field";
import { ToolsCard } from "@/components/settings/ToolsCard";
import { QueueSettingsCard } from "@/components/settings/QueueSettingsCard";
import { FilePreferencesCard } from "@/components/settings/FilePreferencesCard";
import { BackupsCard } from "@/components/settings/BackupsCard";
import { AuditCard } from "@/components/settings/AuditCard";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiDelete, apiGet, apiPatch, apiPut } from "@/lib/api";
import { pickFolder } from "@/lib/folder";
import { shortPath } from "@/lib/format";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import { cn } from "@/lib/utils";
import { confirmAction } from "@/store/confirm-store";
import { links } from "@/lib/links";
import { toast } from "@/store/toast-store";
import type { Workspace } from "@/lib/types";
import { Modal } from "@/components/ui/modal";
import { TimezoneSelect } from "@/components/ui/timezone-select";
import { fieldInputClass as inputClass } from "@/lib/ui-classes";

export function SettingsPage() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const selectWorkspace = useAppStore((state) => state.selectWorkspace);
  const refresh = useAppStore((state) => state.refresh);
  const [editing, setEditing] = useState<Workspace | null>(null);
  const [params, setParams] = useSearchParams();
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );

  const [root, setRoot] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const section = params.get("section") ?? "general";
  const tabs = [
    { id: "general", label: "Geral", icon: Settings, group: "Geral" },
    { id: "engine", label: "Engine", icon: Cpu, group: "Motor" },
    { id: "updates", label: "Atualizações", icon: Download, group: "Motor" },
    { id: "notifications", label: "Notificações", icon: Bell, group: "Operação" },
    { id: "queue", label: "Fila", icon: ListChecks, group: "Operação" },
    { id: "backups", label: "Backups", icon: Archive, group: "Operação" },
    { id: "audit", label: "Auditoria", icon: ShieldCheck, group: "Operação" },
    { id: "workspaces", label: "Workspaces", icon: FolderOpen, group: "Workspace" },
    { id: "tools", label: "Ferramentas", icon: Wrench, group: "Workspace" },
    { id: "about", label: "Sobre", icon: Info, group: "Sobre" },
  ] as const;
  const activeTab = tabs.some((tab) => tab.id === section) ? section : "general";
  function selectTab(id: string) {
    setParams({ section: id }, { replace: true });
  }

  useEffect(() => {
    apiGet<{ workspaces_root: string }>("/api/app/defaults")
      .then((defaults) => setRoot(defaults.workspaces_root))
      .catch((cause) => setError(cause instanceof Error ? cause.message : String(cause)))
      .finally(() => setLoading(false));
  }, []);

  async function chooseRoot() {
    const selected = await pickFolder(
      "Pasta padrão para novos workspaces",
      root || undefined,
    );
    if (selected) {
      setRoot(selected);
      setSaved(false);
    }
  }

  async function saveRoot() {
    if (!root.trim()) {
      setError("Informe uma pasta válida.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const value = await apiPut<string>("/api/settings/workspaces_root", root.trim());
      setRoot(value);
      setSaved(true);
      window.setTimeout(() => setSaved(false), 2500);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function reveal(path: string) {
    try {
      await openPath(path);
    } catch (cause) {
      console.warn("OpenDesk: could not open path", cause);
    }
  }

  return (
    <PageBody>
      <PageHeader
        title="Configurações"
        description="Pasta padrão para novos workspaces e visão dos workspaces registrados. O OpenDesk não move nem renomeia pastas existentes."
        actions={
          <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
            <FolderPlus size={14} />
            Novo workspace
          </Button>
        }
      />

      <div className="grid gap-4 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-start">
        <SectionNav
          items={tabs}
          value={activeTab}
          onChange={selectTab}
          ariaLabel="Seções de configuração"
        />
        <div className="min-w-0 space-y-4">
      {activeTab === "engine" ? (
        <div id="settings-engine">
          <EngineCard />
        </div>
      ) : null}
      {activeTab === "updates" ? (
        <div id="settings-updates">
          <UpdatesCard />
        </div>
      ) : null}
      {activeTab === "about" ? (
        <div id="settings-about">
          <AboutCard />
        </div>
      ) : null}
      {activeTab === "notifications" ? <NotificationCard /> : null}
      {activeTab === "queue" ? <QueueSettingsCard /> : null}
      {activeTab === "backups" ? <BackupsCard /> : null}
      {activeTab === "audit" ? <AuditCard /> : null}
      {activeTab === "tools" ? <ToolsCard /> : null}

      {activeTab === "general" ? (
      <div id="settings-general" className="grid scroll-mt-14 gap-3 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <HardDrive size={15} className="text-zinc-500" />
                Pasta padrão de workspaces
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                Novos workspaces são criados como <span className="font-mono">{"{raiz}/{slug-do-nome}"}</span>.
              </p>
            </div>
            {saved ? (
              <Badge variant="green">
                <Check size={10} />
                salvo
              </Badge>
            ) : null}
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex gap-2">
              <div className="relative min-w-0 flex-1">
                <FolderOpen
                  size={15}
                  className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500"
                />
                <input
                  value={root}
                  onChange={(event) => {
                    setRoot(event.target.value);
                    setSaved(false);
                  }}
                  placeholder="~/Documents/OpenDesk Workareas"
                  className={cn(
                    "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 pl-9 pr-3 font-mono text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60",
                    loading && "opacity-60",
                  )}
                />
              </div>
              <Button variant="outline" size="sm" className="h-9" onClick={() => void chooseRoot()}>
                Escolher
              </Button>
              <Button size="sm" className="h-9" onClick={() => void saveRoot()} disabled={saving || loading}>
                {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
                Salvar
              </Button>
            </div>
            <div className="flex items-center gap-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void reveal(root)}
                disabled={!root}
              >
                <ExternalLink size={12} />
                Abrir pasta
              </Button>
              <p className="text-[11.5px] text-zinc-500">
                A pasta é criada automaticamente se não existir.
              </p>
            </div>
            {error ? (
              <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
                {error}
              </p>
            ) : null}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <Settings size={15} className="text-zinc-500" />
                Segundo plano
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                O OpenDesk continua executando fila, cron e notificações com a janela
                escondida.
              </p>
            </div>
            <Badge variant="green">bandeja</Badge>
          </CardHeader>
          <CardContent className="space-y-2 text-[13px] leading-relaxed text-zinc-500">
            <p>
              Fechar pelo “X” mantém o app na bandeja por padrão. Para encerrar de
              verdade, use <span className="text-zinc-300">Sair</span> no menu do
              ícone da bandeja.
            </p>
            <p>
              O comportamento do “X” pode ser alterado em{" "}
              <button
                type="button"
                onClick={() => selectTab("notifications")}
                className="text-blue-300 underline-offset-2 hover:underline"
              >
                Notificações
              </button>
              .
            </p>
          </CardContent>
        </Card>
        <FilePreferencesCard />
      </div>

      ) : null}

      {activeTab === "workspaces" ? (
      <Card id="settings-workspaces" className="scroll-mt-14">
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Info size={15} className="text-zinc-500" />
              Workspaces registrados
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Você pode registrar pastas de qualquer local do disco.
            </p>
          </div>
          <Badge variant="default">{workspaces.length}</Badge>
        </CardHeader>
        <CardContent>
          {workspaces.length === 0 ? (
            <EmptyState
              icon={FolderPlus}
              title="Nenhum workspace registrado"
              description="Crie uma pasta nova na raiz padrão ou adicione uma pasta existente."
              action={
                <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
                  <FolderPlus size={14} />
                  Adicionar workspace
                </Button>
              }
            />
          ) : (
            <div className="space-y-2">
              {workspaces.map((workspace) => {
                const active = workspace.id === activeWorkspaceId;
                return (
                  <div
                    key={workspace.id}
                    className={cn(
                      "flex flex-wrap items-center gap-3 rounded-lg border px-3 py-2.5",
                      active
                        ? "workspace-selected"
                        : "border-zinc-800/70 bg-zinc-950/40",
                    )}
                  >
                    <span
                      className="h-2.5 w-2.5 shrink-0 rounded-full"
                      style={{ background: workspace.color_hex }}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <p className="truncate text-[13.5px] font-medium text-zinc-200">
                          {workspace.name}
                        </p>
                        {active ? <Badge variant="accent">ativo</Badge> : null}
                        {workspace.root_path.startsWith("/tmp/") ||
                        workspace.root_path.startsWith("/var/tmp/") ? (
                          <Badge variant="yellow">temporário</Badge>
                        ) : null}
                      </div>
                      <p className="truncate font-mono text-[11.5px] text-zinc-500">
                        {shortPath(workspace.root_path, 80)}
                      </p>
                      {workspace.root_path.startsWith("/tmp/") ||
                      workspace.root_path.startsWith("/var/tmp/") ? (
                        <p className="mt-0.5 text-[11.5px] leading-relaxed text-amber-300/80">
                          Pasta temporária: edite a raiz para um diretório em
                          Documentos/OpenDesk Workareas para preservar os arquivos.
                        </p>
                      ) : null}
                    </div>
                    <Badge variant="default">
                      {ROUTER_POLICY_LABELS[workspace.default_router_policy] ??
                        workspace.default_router_policy}
                    </Badge>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7"
                      onClick={() => setEditing(workspace)}
                    >
                      Editar
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7"
                      onClick={() => void reveal(workspace.root_path)}
                    >
                      <ExternalLink size={12} />
                      Abrir
                    </Button>
                    {!active ? (
                      <Button
                        size="sm"
                        className="h-7"
                        onClick={() => void selectWorkspace(workspace.id)}
                      >
                        Ativar
                      </Button>
                    ) : null}
                    <Button
                      variant="destructive"
                      size="sm"
                      className="h-7"
                      onClick={async () => {
                        if (
                          !(await confirmAction({
                            title: `Remover o workspace ${workspace.name}?`,
                            description:
                              "O registro é removido do OpenDesk. Nenhum arquivo ou pasta do disco é apagado.",
                            confirmLabel: "Remover registro",
                            tone: "destructive",
                          }))
                        ) {
                          return;
                        }
                        try {
                          await apiDelete(`/api/workspaces/${workspace.id}`);
                          await refresh();
                          toast({
                            tone: "success",
                            title: "Workspace removido",
                            description: "A pasta original continua intacta no disco.",
                            href: links.settings("workspaces"),
                          });
                        } catch (cause) {
                          toast({
                            tone: "error",
                            title: "Não foi possível remover o workspace",
                            description: cause instanceof Error ? cause.message : String(cause),
                          });
                        }
                      }}
                    >
                      Remover
                    </Button>
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      ) : null}
        </div>
      </div>

      <WorkspaceEditModal
        workspace={editing}
        onClose={() => setEditing(null)}
        onSaved={async () => {
          setEditing(null);
          await refresh();
          toast({
            tone: "success",
            title: "Workspace atualizado",
            href: links.settings("workspaces"),
          });
        }}
      />
    </PageBody>
  );
}

function WorkspaceEditModal({
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



