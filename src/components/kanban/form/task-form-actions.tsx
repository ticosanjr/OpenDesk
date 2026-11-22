import { ArrowRightLeft, Loader2, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";

export function TaskFormActions({
  mode,
  saving,
  running,
  onSubmit,
  onCancel,
}: {
  mode: "create" | "edit" | "fork";
  saving: "save" | "rerun" | null;
  running: boolean;
  onSubmit: (rerun: boolean) => void;
  onCancel: () => void;
}) {
  return (
    <div className="mt-4 flex flex-wrap justify-end gap-2 border-t border-zinc-800/70 pt-4">
      <Button variant="ghost" size="sm" onClick={onCancel}>
        Cancelar
      </Button>
      {mode === "create" || mode === "fork" ? (
        <Button size="sm" onClick={() => onSubmit(false)} disabled={saving !== null}>
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          {mode === "fork" ? "Criar fork" : "Criar tarefa"}
        </Button>
      ) : (
        <>
          <Button
            variant="outline"
            size="sm"
            onClick={() => onSubmit(false)}
            disabled={saving !== null || running}
          >
            {saving === "save" ? <Loader2 size={14} className="animate-spin" /> : null}
            Salvar
          </Button>
          <Button
            size="sm"
            onClick={() => onSubmit(true)}
            disabled={saving !== null || running}
          >
            {saving === "rerun" ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <ArrowRightLeft size={14} />
            )}
            Salvar e reexecutar
          </Button>
        </>
      )}
    </div>
  );
}
