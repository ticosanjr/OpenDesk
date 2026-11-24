import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  Archive,
  Bell,
  Cpu,
  Download,
  FileCode2,
  FolderOpen,
  FolderPlus,
  Fingerprint,
  Info,
  ListChecks,
  Network,
  Volume2,
  ShieldCheck,
  Settings,
  Workflow,
  Wrench,
} from "lucide-react";
import { openLocalPath } from "@/lib/runtime-env";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { EngineCard } from "@/components/settings/EngineCard";
import { HarnessCard } from "@/components/settings/HarnessCard";
import { SandboxComputerCard } from "@/components/settings/SandboxComputerCard";
import { VoiceCard } from "@/components/settings/VoiceCard";
import { NodesCard } from "@/components/settings/NodesCard";
import { PasskeyCard } from "@/components/settings/PasskeyCard";
import { UpdatesCard } from "@/components/settings/UpdatesCard";
import { AboutCard } from "@/components/settings/AboutCard";
import { NotificationCard } from "@/components/settings/NotificationCard";
import { SectionNav } from "@/components/ui/section-nav";
import { ToolsCard } from "@/components/settings/ToolsCard";
import { QueueSettingsCard } from "@/components/settings/QueueSettingsCard";
import { BackupsCard } from "@/components/settings/BackupsCard";
import { ConfigBundleCard } from "@/components/settings/ConfigBundleCard";
import { AuditCard } from "@/components/settings/AuditCard";
import { PromptSettings } from "@/components/settings/PromptSettings";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet, apiPut } from "@/lib/api";
import { pickFolder } from "@/lib/folder";
import { links } from "@/lib/links";
import { toast } from "@/store/toast-store";
import type { Workspace } from "@/lib/types";
import { GeneralSettings } from "./settings/GeneralSettings";
import { WorkspaceEditModal } from "./settings/WorkspaceEditModal";
import { WorkspacesCard } from "./settings/WorkspacesCard";

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
    { id: "harness", label: "Harnesses", icon: Workflow, group: "Motor" },
    { id: "sandbox", label: "Sandbox", icon: ShieldCheck, group: "Motor" },
    { id: "voice", label: "Voz", icon: Volume2, group: "Operação" },
    { id: "nodes", label: "Nós", icon: Network, group: "Operação" },
    { id: "security", label: "Segurança", icon: Fingerprint, group: "Operação" },
    { id: "updates", label: "Atualizações", icon: Download, group: "Motor" },
    { id: "prompt", label: "Prompt", icon: FileCode2, group: "Motor" },
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
    const result = await openLocalPath(path);
    if (!result.ok) {
      toast({
        tone: "info",
        title: "Caminho local",
        description: result.fallback ?? path,
      });
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
      {activeTab === "prompt" ? (
        <div id="settings-prompt">
          <PromptSettings />
        </div>
      ) : null}
      {activeTab === "harness" ? <HarnessCard /> : null}
      {activeTab === "sandbox" && activeWorkspaceId ? (
        <SandboxComputerCard workspaceId={activeWorkspaceId} />
      ) : null}
      {activeTab === "voice" ? <VoiceCard /> : null}
      {activeTab === "nodes" ? <NodesCard /> : null}
      {activeTab === "security" ? <PasskeyCard /> : null}
      {activeTab === "notifications" ? <NotificationCard /> : null}
      {activeTab === "queue" ? <QueueSettingsCard /> : null}
      {activeTab === "backups" ? <BackupsCard /> : null}
      {activeTab === "audit" ? <AuditCard /> : null}
      {activeTab === "tools" ? <ToolsCard /> : null}

      {activeTab === "general" ? (
        <>
        <GeneralSettings
          root={root}
          loading={loading}
          saving={saving}
          saved={saved}
          error={error}
          onRootChange={(value) => {
            setRoot(value);
            setSaved(false);
          }}
          onChooseRoot={() => void chooseRoot()}
          onSaveRoot={() => void saveRoot()}
          onReveal={() => void reveal(root)}
          onSelectTab={selectTab}
        />
        <ConfigBundleCard />
        </>
      ) : null}

      {activeTab === "workspaces" ? (
        <WorkspacesCard
          workspaces={workspaces}
          activeWorkspaceId={activeWorkspaceId}
          onEdit={setEditing}
          onSelect={selectWorkspace}
          onReveal={(path) => void reveal(path)}
          onRefresh={refresh}
          onOpenDialog={() => setWorkspaceDialogOpen(true)}
        />
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
