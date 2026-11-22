import { FolderPlus } from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";

export function EmptyWorkspace({ onCreate }: { onCreate: () => void }) {
  return (
    <PageBody>
      <PageHeader
        title="Dashboard"
        description="Registre um workspace para começar a governar agentes, runs, artefatos e custos."
        actions={
          <Button size="sm" onClick={onCreate}>
            <FolderPlus size={15} />
            Novo workspace
          </Button>
        }
      />
      <EmptyState
        icon={FolderPlus}
        title="Nenhum workspace registrado"
        description="O OpenDesk guarda a configuração do OpenCode nos arquivos nativos. O workspace só precisa apontar para o diretório do projeto."
        action={
          <Button size="sm" onClick={onCreate}>
            <FolderPlus size={15} />
            Criar primeiro workspace
          </Button>
        }
      />
    </PageBody>
  );
}
