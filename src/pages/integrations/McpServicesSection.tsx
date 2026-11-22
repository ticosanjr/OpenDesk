import {
  CalendarDays,
  FileText,
  HardDrive,
  ListTodo,
  Mail,
  Plug,
  ShieldCheck,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

const GOOGLE_SERVICES = [
  { icon: FileText, title: "Docs", description: "Markdown → documento nativo formatado" },
  { icon: HardDrive, title: "Drive", description: "Upload e organização por workspace" },
  { icon: CalendarDays, title: "Calendar", description: "Prazos e eventos a partir dos artefatos" },
  { icon: Mail, title: "Gmail", description: "Envio, rascunhos e leitura de threads" },
  { icon: ListTodo, title: "Tasks", description: "Backlog bidirecional (Keep só enterprise)" },
];

export function McpServicesSection() {
  return (
    <>
      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Plug size={15} className="text-zinc-500" />
              Serviços e ferramentas MCP
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              O mesmo executável expõe <span className="font-mono">opendesk mcp-google</span> via
              stdio e registra em <span className="font-mono">mcp.servers</span>.
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
                <p className="mt-2 text-[13px] font-medium text-zinc-300">{service.title}</p>
                <p className="mt-1 text-[11.5px] leading-relaxed text-zinc-500">
                  {service.description}
                </p>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="flex items-start gap-3 p-4">
          <ShieldCheck size={16} className="mt-0.5 text-emerald-400/70" />
          <p className="text-[13px] leading-relaxed text-zinc-500">
            Segredos são criptografados com AES-256-GCM e chave derivada da máquina,
            gravados em <span className="font-mono">~/.local/share/opendesk/secrets</span> com
            permissão 0600. Logs e eventos nunca incluem senhas.
          </p>
        </CardContent>
      </Card>
    </>
  );
}
