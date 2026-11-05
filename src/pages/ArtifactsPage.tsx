import { Archive, Filter, GitCompare, Mail, Search } from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";

export function ArtifactsPage() {
  return (
    <PageBody>
      <PageHeader
        eyebrow="proveniência"
        title="Artefatos"
        description="Onde está o que os agentes produziram, quem fez, por quê, quando e para onde foi enviado."
        actions={
          <Badge variant="default">
            <Archive size={10} />
            fase 3
          </Badge>
        }
      />

      <Card>
        <CardHeader className="gap-3">
          <CardTitle>Explorador de proveniência</CardTitle>
          <div className="flex flex-wrap items-center gap-2">
            <div className="flex h-8 min-w-[200px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5 opacity-60">
              <Search size={13} className="text-zinc-600" />
              <input
                disabled
                placeholder="Filtrar por arquivo, agente ou tarefa…"
                className="h-full flex-1 bg-transparent text-[11px] outline-none placeholder:text-zinc-600"
              />
            </div>
            <Button variant="outline" size="sm" disabled>
              <Filter size={13} />
              Filtros
            </Button>
            <Button variant="outline" size="sm" disabled>
              <Mail size={13} />
              Enviar
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <EmptyState
            icon={Archive}
            title="Nenhum artefato registrado"
            description="Quando um agente criar ou modificar arquivos, o stream interceptor grava autor, motivo, timestamp e diff em artifacts_provenance."
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <GitCompare size={15} className="text-zinc-500" />
            O que será rastreado
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-2 md:grid-cols-2 xl:grid-cols-4">
            {[
              ["Arquivo", "Caminho relativo e absoluto, tipo e tamanho"],
              ["Autor", "Agente ou subagente que executou a tool"],
              ["Motivo", "Tarefa e resumo imediatamente anterior à escrita"],
              ["Destinos", "E-mail, Google Docs e Drive com URL de retorno"],
            ].map(([title, description]) => (
              <div
                key={title}
                className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3"
              >
                <p className="text-[11px] font-medium text-zinc-300">{title}</p>
                <p className="mt-1 text-[10px] leading-relaxed text-zinc-600">
                  {description}
                </p>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </PageBody>
  );
}
