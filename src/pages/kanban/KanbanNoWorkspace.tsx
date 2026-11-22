import { ListChecks, Plus } from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";

export function KanbanNoWorkspace({
  onCreateWorkspace,
}: {
  onCreateWorkspace: () => void;
}) {
  return (
    <PageBody>
      <PageHeader
        title="Kanban & Fila"
        description="Crie um workspace para planejar, executar e auditar tarefas."
      />
      <EmptyState
        icon={ListChecks}
        title="Nenhum workspace ativo"
        description="O Kanban opera sobre o workspace selecionado."
        action={
          <Button size="sm" onClick={onCreateWorkspace}>
            <Plus size={14} />
            Criar workspace
          </Button>
        }
      />
    </PageBody>
  );
}
