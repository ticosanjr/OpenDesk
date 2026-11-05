import {
  CalendarDays,
  Cloud,
  FileText,
  HardDrive,
  KeyRound,
  ListTodo,
  Mail,
  Plug,
  ShieldCheck,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useAppStore } from "@/store/app-store";

const GOOGLE_SERVICES = [
  { icon: FileText, title: "Docs", description: "Markdown → documento nativo formatado" },
  { icon: HardDrive, title: "Drive", description: "Upload e organização por workspace" },
  { icon: CalendarDays, title: "Calendar", description: "Prazos e eventos a partir dos artefatos" },
  { icon: Mail, title: "Gmail", description: "Envio, rascunhos e leitura de threads" },
  { icon: ListTodo, title: "Tasks", description: "Backlog bidirecional (Keep só enterprise)" },
];

export function IntegrationsPage() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  return (
    <PageBody>
      <PageHeader
        eyebrow="credenciais por workspace"
        title="Integrações"
        description="SMTP e Google Workspace. Tokens e senhas ficam no keyring do sistema, nunca no SQLite em texto puro."
        actions={<Badge variant="default">fase 5</Badge>}
      />

      <div className="grid gap-3 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <Mail size={15} className="text-zinc-500" />
                SMTP
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                Envio de resumos e anexos ao concluir tarefas e rotinas.
              </p>
            </div>
            <Badge variant="default">configurar</Badge>
          </CardHeader>
          <CardContent className="space-y-3">
            <InfoRow label="Workspace" value={workspace?.name ?? "nenhum"} />
            <InfoRow label="Host" value="não configurado" />
            <InfoRow label="Porta" value="587" />
            <InfoRow label="TLS" value="obrigatório" />
            <InfoRow label="Remetente" value="não configurado" />
            <div className="flex gap-2 pt-1">
              <Button size="sm" disabled>
                Configurar SMTP
              </Button>
              <Button variant="outline" size="sm" disabled>
                Testar conexão
              </Button>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <Cloud size={15} className="text-zinc-500" />
                Google Workspace
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                OAuth2 loopback + PKCE; refresh token no keyring.
              </p>
            </div>
            <Badge variant="default">não conectado</Badge>
          </CardHeader>
          <CardContent className="space-y-3">
            <InfoRow label="Conta" value="não conectada" />
            <InfoRow label="Calendar" value="primary" />
            <InfoRow label="Drive folder" value="não configurada" />
            <div className="flex gap-2 pt-1">
              <Button size="sm" disabled>
                <KeyRound size={13} />
                Conectar Google
              </Button>
              <Button variant="outline" size="sm" disabled>
                Revogar
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Plug size={15} className="text-zinc-500" />
              Serviços e ferramentas MCP
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              O mesmo executável expõe <span className="font-mono">opendesk mcp-google</span>{" "}
              via stdio e registra o servidor em <span className="font-mono">mcp.servers</span>.
            </p>
          </div>
          <Badge variant="purple">MCP bridge</Badge>
        </CardHeader>
        <CardContent>
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-5">
            {GOOGLE_SERVICES.map((service) => (
              <div
                key={service.title}
                className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3"
              >
                <service.icon size={15} className="text-zinc-500" />
                <p className="mt-2 text-[11px] font-medium text-zinc-300">
                  {service.title}
                </p>
                <p className="mt-1 text-[9px] leading-relaxed text-zinc-600">
                  {service.description}
                </p>
              </div>
            ))}
          </div>
          <p className="mt-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[10px] leading-relaxed text-zinc-600">
            Google Keep não possui API para contas comuns. O backlog usa Google
            Tasks; Keep fica atrás de uma flag enterprise com service account.
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="flex items-start gap-3 p-4">
          <ShieldCheck size={16} className="mt-0.5 text-emerald-400/70" />
          <p className="text-[11px] leading-relaxed text-zinc-500">
            Segredos são gravados no keyring do SO com fallback AES-GCM por máquina
            quando o Secret Service não estiver disponível. Logs e eventos nunca
            incluem tokens, senhas ou prompts completos.
          </p>
        </CardContent>
      </Card>
    </PageBody>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 text-[11px]">
      <span className="text-zinc-600">{label}</span>
      <span className="font-mono text-[10px] text-zinc-300">{value}</span>
    </div>
  );
}
