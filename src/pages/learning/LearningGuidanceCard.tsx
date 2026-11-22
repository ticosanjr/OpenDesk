import { Card, CardContent } from "@/components/ui/card";

export function LearningGuidanceCard() {
  return (
    <Card className="border-violet-500/20 bg-violet-500/[0.025]">
      <CardContent className="grid gap-3 p-4 text-[12px] leading-relaxed text-zinc-400 sm:grid-cols-2">
        <div>
          <p className="text-[13px] font-medium text-violet-200">
            Promover = criar uma proposta
          </p>
          <p className="mt-1">
            Gera uma proposta de skill no Studio. Nada é gravado no workspace
            até você revisar e aprovar a proposta.
          </p>
        </div>
        <div>
          <p className="text-[13px] font-medium text-emerald-200">
            Memorizar = salvar na memória
          </p>
          <p className="mt-1">
            Adiciona um bloco em <span className="font-mono">.opencode/MEMORY.md</span>.
            Esse conteúdo entra no contexto das próximas execuções e pode ser
            removido depois pelo botão “Remover da memória”.
          </p>
        </div>
      </CardContent>
    </Card>
  );
}
