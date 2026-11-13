import type { WorkspaceTemplate } from "./pack-types";

export function WorkspaceTemplateCard({
  template,
  action,
  onClick,
}: {
  template: WorkspaceTemplate;
  action?: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 text-left transition-colors hover:border-zinc-600"
    >
      <div className="flex items-center gap-2">
        <span
          className="h-2.5 w-2.5 rounded-full"
          style={{ background: template.color }}
        />
        <span className="text-[13.5px] font-medium text-zinc-200">
          {template.name}
        </span>
      </div>
      <p className="mt-1 text-[12px] leading-snug text-zinc-500">
        {template.description}
      </p>
      <p className="mt-2 text-[12px] text-zinc-500">
        {template.packs.length} pacotes · {template.folders.length} pastas
        {action ? ` · ${action}` : ""}
      </p>
      <p className="mt-1 truncate font-mono text-[11px] text-zinc-500">
        {template.packs.slice(0, 4).join(" · ")}
        {template.packs.length > 4 ? ` +${template.packs.length - 4}` : ""}
      </p>
    </button>
  );
}
