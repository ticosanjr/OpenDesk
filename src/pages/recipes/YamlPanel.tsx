import { ChevronDown, ChevronRight, Loader2, Save } from "lucide-react";
import { LocalYamlEditor } from "@/components/recipes/recipe-editors";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { FieldHint } from "@/components/ui/field-hint";
import { IconLabel } from "@/components/ui/icon-label";

export function YamlPanel({
  open,
  yaml,
  canSave,
  saving,
  onChange,
  onSave,
  onOpen,
  onClose,
  onResizeStart,
  onResetWidth,
}: {
  open: boolean;
  yaml: string;
  canSave: boolean;
  saving: boolean;
  onChange: (value: string) => void;
  onSave: () => void;
  onOpen: () => void;
  onClose: () => void;
  onResizeStart: () => void;
  onResetWidth: () => void;
}) {
  return (
    <>
      {open ? (
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label="Redimensionar editor YAML"
          onPointerDown={onResizeStart}
          onDoubleClick={onResetWidth}
          className="hidden w-2 shrink-0 cursor-col-resize items-center justify-center rounded-full bg-zinc-800/40 transition-colors hover:bg-blue-500/30 xl:flex"
          title="Arraste para redimensionar; clique duplo restaura"
        >
          <span className="h-8 w-0.5 rounded-full bg-zinc-600" />
        </div>
      ) : null}

      {open ? (
        <Card className="flex min-h-0 w-full flex-col overflow-hidden xl:w-[var(--yaml-width)]">
          <CardHeader className="py-3">
            <div className="flex min-w-0 items-center gap-2">
              <button
                type="button"
                onClick={onClose}
                aria-expanded={true}
                aria-label="Recolher editor YAML"
                className="flex items-center gap-2 text-left"
                title="Recolher editor YAML"
              >
                <ChevronDown size={15} />
                <CardTitle>YAML</CardTitle>
              </button>
              <FieldHint hint="Editor local. Arraste a divisória para redimensionar." />
            </div>
            <Button size="sm" onClick={onSave} disabled={!canSave || saving}>
              <IconLabel icon={saving ? <Loader2 size={12} className="animate-spin" /> : <Save size={12} />}>
                Salvar
              </IconLabel>
            </Button>
          </CardHeader>
          <CardContent className="h-[70dvh] p-0">
            <LocalYamlEditor value={yaml} onChange={onChange} />
          </CardContent>
        </Card>
      ) : (
        <button
          type="button"
          onClick={onOpen}
          aria-label="Expandir editor YAML"
          className="flex min-h-11 items-center justify-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/30 px-3 py-2 text-[12.5px] text-zinc-400 transition-colors hover:border-zinc-700 hover:text-zinc-200 xl:h-[70dvh] xl:w-11 xl:flex-col xl:px-0"
          title="Abrir editor YAML"
        >
          <ChevronRight size={13} className="xl:rotate-0" />
          <span className="xl:[writing-mode:vertical-rl]">YAML</span>
        </button>
      )}
    </>
  );
}
