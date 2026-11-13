import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  BookOpen,
  Bot,
  FileJson,
  FolderPlus,
  Plug,
  Terminal,
  Wand2,
} from "lucide-react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { AgentPanel } from "@/components/studio/AgentPanel";
import { SkillPanel } from "@/components/studio/SkillPanel";
import { CommandPanel } from "@/components/studio/CommandPanel";
import { McpPanel } from "@/components/studio/McpPanel";
import { ConfigPanel } from "@/components/studio/ConfigPanel";
import { StudioOverview } from "@/components/studio/StudioOverview";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import type { StudioScope } from "@/lib/types";
import { cn } from "@/lib/utils";

type Tab = "agents" | "skills" | "commands" | "mcp" | "config";

const TABS: { id: Tab; label: string; icon: typeof Bot }[] = [
  { id: "agents", label: "Agentes", icon: Bot },
  { id: "skills", label: "Skills", icon: BookOpen },
  { id: "commands", label: "Comandos", icon: Terminal },
  { id: "mcp", label: "MCP", icon: Plug },
  { id: "config", label: "JSONC bruto", icon: FileJson },
];

export function StudioPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const [tab, setTab] = useState<Tab>("agents");
  const [scope, setScope] = useState<StudioScope | "effective">("effective");
  const [focusId, setFocusId] = useState<string | null>(null);
  const [params, setParams] = useSearchParams();
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  useEffect(() => {
    const id = params.get("id");
    if (!id) return;
    const kind = params.get("kind");
    const requested = params.get("scope");
    if (requested === "global" || requested === "workspace") {
      setScope(requested);
    }
    setFocusId(id);
    setTab(
      kind === "skill"
        ? "skills"
        : kind === "command"
          ? "commands"
          : kind === "mcp"
            ? "mcp"
            : "agents",
    );
    setParams({}, { replace: true });
  }, [params, setParams]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="px-5 pb-0 pt-5 lg:px-7 lg:pt-7">
        <PageHeader
          title="OpenCode Studio"
          description="CRUD visual de agentes, skills, comandos, MCPs e JSONC — sempre gravando nos arquivos nativos do OpenCode."
          actions={
            <>
              <Badge variant="blue">SSOT no disco</Badge>
              <Badge variant="green">
                <Wand2 size={10} />
                disco + runtime
              </Badge>
            </>
          }
        />

        <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
          <div
            role="group"
            aria-label="Escopo do Studio"
            className="flex items-center gap-1 rounded-lg border border-zinc-800/80 bg-zinc-900/30 p-1"
          >
            <button
              type="button"
              onClick={() => setScope("workspace")}
              className={cn(
                "rounded-md px-3 py-1.5 text-[13.5px] font-medium transition-colors",
                scope === "workspace"
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:text-zinc-200",
              )}
            >
              Workspace{workspace ? ` · ${workspace.name}` : ""}
            </button>
            <button
              type="button"
              onClick={() => setScope("global")}
              className={cn(
                "rounded-md px-3 py-1.5 text-[13.5px] font-medium transition-colors",
                scope === "global"
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:text-zinc-200",
              )}
            >
              Global
            </button>
            <button
              type="button"
              onClick={() => setScope("effective")}
              className={cn(
                "rounded-md px-3 py-1.5 text-[13.5px] font-medium transition-colors",
                scope === "effective"
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:text-zinc-200",
              )}
            >
              Efetivo
            </button>
          </div>

          <div className={cn("flex flex-wrap items-center gap-1", scope === "effective" && "hidden")}>
            {TABS.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setTab(item.id)}
                className={cn(
                  "flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[14px] font-medium transition-colors",
                  tab === item.id
                    ? "bg-zinc-800 text-zinc-100"
                    : "text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200",
                )}
              >
                <item.icon size={15} />
                {item.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="mt-3 flex min-h-0 flex-1 flex-col overflow-hidden border-t border-zinc-800/70">
        {scope === "effective" ? (
          <StudioOverview
            workspaceId={activeWorkspaceId}
            onOpenEditor={(kind, targetScope, id) => {
              setScope(targetScope);
              setFocusId(id);
              setTab(kind === "mcp" ? "mcp" : kind === "skill" ? "skills" : kind === "command" ? "commands" : "agents");
            }}
          />
        ) : scope === "workspace" && !activeWorkspaceId ? (
          <div className="p-5 lg:p-7">
            <EmptyState
              icon={FolderPlus}
              title="Nenhum workspace ativo"
              description="Selecione um workspace na barra superior ou crie um para editar .opencode/ e opencode.jsonc. O escopo Global continua disponível sem workspace."
              action={
                <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
                  <FolderPlus size={14} />
                  Criar workspace
                </Button>
              }
            />
          </div>
        ) : tab === "agents" ? (
          <AgentPanel scope={scope as StudioScope} workspaceId={activeWorkspaceId} focusId={focusId} />
        ) : tab === "skills" ? (
          <SkillPanel scope={scope as StudioScope} workspaceId={activeWorkspaceId} focusId={focusId} />
        ) : tab === "commands" ? (
          <CommandPanel scope={scope as StudioScope} workspaceId={activeWorkspaceId} focusId={focusId} />
        ) : tab === "mcp" ? (
          <McpPanel scope={scope as StudioScope} workspaceId={activeWorkspaceId} focusId={focusId} />
        ) : (
          <ConfigPanel scope={scope as StudioScope} workspaceId={activeWorkspaceId} />
        )}
      </div>
    </div>
  );
}
