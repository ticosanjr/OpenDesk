import {
  Bot,
  BookOpen,
  FileCode2,
  FileJson,
  Plug,
  ShieldCheck,
  Terminal,
  Wand2,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { shortPath } from "@/lib/format";

export function StudioPage() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  return (
    <PageBody>
      <PageHeader
        eyebrow="espelho bidirecional"
        title="OpenCode Studio"
        description="CRUD visual de agentes, skills, comandos, MCPs e permissões — sempre gravando nos arquivos nativos do OpenCode."
        actions={<Badge variant="default">fase 2</Badge>}
      />

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        {[
          {
            icon: Bot,
            title: "Agentes & Subagentes",
            description: "Frontmatter YAML, modelo, modo, permissões e system prompt.",
          },
          {
            icon: BookOpen,
            title: "Skills",
            description: "SKILL.md por diretório, arquivos de referência e scripts.",
          },
          {
            icon: Terminal,
            title: "Comandos /",
            description: "Comandos em .opencode/commands/ e no bloco commands do JSONC.",
          },
          {
            icon: Plug,
            title: "MCP Servers",
            description: "mcp.servers local/remote, ambiente, disabled e teste de conexão.",
          },
        ].map((item) => (
          <Card key={item.title}>
            <CardContent className="p-4">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/60 text-zinc-500">
                <item.icon size={15} strokeWidth={1.7} />
              </div>
              <p className="mt-3 text-[12px] font-medium text-zinc-200">
                {item.title}
              </p>
              <p className="mt-1 text-[11px] leading-relaxed text-zinc-500">
                {item.description}
              </p>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="grid gap-3 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <FolderPlusIcon />
              Escopos monitorados
            </CardTitle>
            <Badge variant="blue">SSOT</Badge>
          </CardHeader>
          <CardContent className="space-y-3">
            <ScopeRow
              label="Global"
              path="~/.config/opencode"
              detail="opencode.jsonc, agents/, skills/, commands/"
            />
            <ScopeRow
              label="Workspace"
              path={workspace ? shortPath(workspace.root_path, 52) : "sem workspace"}
              detail=".opencode/ e opencode.jsonc"
            />
            <p className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[10px] leading-relaxed text-zinc-600">
              O banco guarda apenas estado operacional. Nenhum agente, skill ou
              comando é espelhado no SQLite.
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ShieldCheck size={15} className="text-zinc-500" />
              Permissões globais
            </CardTitle>
            <Badge variant="default">V2</Badge>
          </CardHeader>
          <CardContent className="space-y-2">
            {[
              ["edit", "allow", "Editar e escrever arquivos"],
              ["shell", "ask", "Comandos no terminal"],
              ["webfetch", "allow", "Buscar conteúdo externo"],
            ].map(([action, effect, description]) => (
              <div
                key={action}
                className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
              >
                <div>
                  <p className="font-mono text-[10px] text-zinc-300">{action}</p>
                  <p className="text-[9px] text-zinc-600">{description}</p>
                </div>
                <Badge
                  variant={
                    effect === "allow" ? "green" : effect === "ask" ? "yellow" : "red"
                  }
                >
                  {effect}
                </Badge>
              </div>
            ))}
            <p className="pt-1 text-[10px] leading-relaxed text-zinc-600">
              O editor real compila a matriz visual para a lista ordenada{" "}
              <span className="font-mono">permissions: [&#123;action, resource, effect&#125;]</span>{" "}
              do OpenCode V2.
            </p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FileJson size={15} className="text-zinc-500" />
            Editor visual
          </CardTitle>
          <Badge variant="default">fase 2</Badge>
        </CardHeader>
        <CardContent>
          <EmptyState
            icon={Wand2}
            title="O editor chega com o parser JSONC e o watcher"
            description="A Fase 2 implementa leitura/escrita cirúrgica do opencode.jsonc preservando comentários, formulários de agentes/skills/comandos e sincronização em menos de 500 ms."
          />
        </CardContent>
      </Card>
    </PageBody>
  );
}

function ScopeRow({
  label,
  path,
  detail,
}: {
  label: string;
  path: string;
  detail: string;
}) {
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[11px] font-medium text-zinc-300">{label}</span>
        <FileCode2 size={13} className="text-zinc-600" />
      </div>
      <p className="mt-1 truncate font-mono text-[10px] text-zinc-500">{path}</p>
      <p className="mt-1 truncate text-[9px] text-zinc-600">{detail}</p>
    </div>
  );
}

function FolderPlusIcon() {
  return <FileCode2 size={15} className="text-zinc-500" />;
}
