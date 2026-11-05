import {
  ArrowUp,
  Bot,
  MessageSquare,
  Paperclip,
  Route,
  Sparkles,
  Wand2,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";

const CAPABILITIES = [
  {
    icon: MessageSquare,
    title: "Chat com streaming",
    description:
      "Sessões do OpenCode com texto, raciocínio e tool calls transmitidos em tempo real.",
  },
  {
    icon: Wand2,
    title: "Meta-Builder",
    description:
      "“Crie um subagente revisor fiscal…” gera o arquivo .opencode/agents/<slug>.md validado e um card pronto para usar.",
  },
  {
    icon: Route,
    title: "Roteador DSH",
    description:
      "Classifica a intenção, decide Plan vs Build, escolhe subagente, modelo e effort antes de executar.",
  },
  {
    icon: Sparkles,
    title: "Context Packer",
    description:
      "Injeta somente caminhos, skills e restrições relevantes, incluindo a regra obrigatória de output.",
  },
];

export function ChatPage() {
  return (
    <PageBody>
      <PageHeader
        eyebrow="interação direta"
        title="Chat & Criador"
        description="Converse com o OpenCode, crie agentes/skills/comandos por linguagem natural e acompanhe a decisão do roteador."
        actions={
          <Badge variant="default">
            <Bot size={10} />
            fase 3
          </Badge>
        }
      />

      <div className="grid gap-3 md:grid-cols-2">
        {CAPABILITIES.map((capability) => (
          <Card key={capability.title}>
            <CardContent className="p-4">
              <div className="flex items-start gap-3">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/60 text-zinc-500">
                  <capability.icon size={15} strokeWidth={1.7} />
                </div>
                <div>
                  <p className="text-[12px] font-medium text-zinc-200">
                    {capability.title}
                  </p>
                  <p className="mt-1 text-[11px] leading-relaxed text-zinc-500">
                    {capability.description}
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Nova mensagem</CardTitle>
          <Badge variant="default">em breve</Badge>
        </CardHeader>
        <CardContent className="space-y-3">
          <EmptyState
            icon={MessageSquare}
            title="O chat será ligado ao daemon na Fase 3"
            description="A fundação já expõe /api/daemon/status, /api/agents e /api/models. A próxima fase adiciona sessões, prompt e eventos SSE."
          />
          <div className="flex items-center gap-2 rounded-xl border border-zinc-800 bg-zinc-950/50 px-3 py-2 opacity-60">
            <Paperclip size={15} className="text-zinc-600" />
            <input
              disabled
              placeholder="Descreva a tarefa ou peça para criar um agente…"
              className="h-8 flex-1 bg-transparent text-[12px] text-zinc-300 outline-none placeholder:text-zinc-600"
            />
            <Button size="icon" disabled className="h-8 w-8">
              <ArrowUp size={15} />
            </Button>
          </div>
        </CardContent>
      </Card>
    </PageBody>
  );
}
