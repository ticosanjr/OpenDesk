import { CalendarClock, Clock3, Mail, Play, Plus, Timer } from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";

export function RoutinesPage() {
  return (
    <PageBody>
      <PageHeader
        eyebrow="automação recorrente"
        title="Rotinas (Cron)"
        description="Agende comandos e prompts, crie pastas determinísticas e dispare e-mail ou Google Docs sem abrir o chat."
        actions={
          <>
            <Badge variant="default">
              <CalendarClock size={10} />
              fase 4
            </Badge>
            <Button size="sm" disabled>
              <Plus size={15} />
              Nova rotina
            </Button>
          </>
        }
      />

      <div className="grid gap-3 md:grid-cols-3">
        {[
          {
            icon: Clock3,
            title: "Agendamento",
            description:
              "Expressão cron, timezone, próxima execução e tolerância a DST.",
          },
          {
            icon: Play,
            title: "Execução determinística",
            description:
              "output/YYYY-MM/<schedule_slug>/ criado antes de enfileirar o job.",
          },
          {
            icon: Mail,
            title: "Entrega",
            description:
              "Resumo e anexos por SMTP/Gmail; Docs, Drive e Calendar opcionais.",
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

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Timer size={15} className="text-zinc-500" />
            Rotinas ativas
          </CardTitle>
          <Badge variant="default">0 agendadas</Badge>
        </CardHeader>
        <CardContent>
          <EmptyState
            icon={CalendarClock}
            title="Nenhuma rotina configurada"
            description="O motor cron dirigido pelo SQLite entra na Fase 4, com idempotência por schedule_id + horário agendado."
          />
        </CardContent>
      </Card>
    </PageBody>
  );
}
