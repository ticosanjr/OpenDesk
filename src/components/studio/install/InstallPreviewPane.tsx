import { Badge } from "@/components/ui/badge";
import type { StudioInstallPreview } from "@/lib/types";
import { inputClass } from "./install-sources";

export function InstallPreviewPane({
  preview,
  selected,
  onToggle,
  scope,
  onScopeChange,
  workspaces,
  targetWorkspace,
  onTargetWorkspaceChange,
  overwrite,
  onOverwriteChange,
}: {
  preview: StudioInstallPreview | null;
  selected: Set<number>;
  onToggle: (index: number, checked: boolean) => void;
  scope: "workspace" | "global";
  onScopeChange: (scope: "workspace" | "global") => void;
  workspaces: Array<{ id: string; name: string }>;
  targetWorkspace: string;
  onTargetWorkspaceChange: (value: string) => void;
  overwrite: boolean;
  onOverwriteChange: (overwrite: boolean) => void;
}) {
  if (!preview) return null;
  return (
    <div className="space-y-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
      <div className="flex items-center gap-2">
        <p className="text-[13px] font-medium text-zinc-200">
          Itens detectados
        </p>
        <Badge variant="blue">{preview.items.length}</Badge>
      </div>
      {preview.items.map((item, index) => (
        <label
          key={`${item.kind}:${item.id}:${index}`}
          className="flex items-center gap-2 text-[13.5px] text-zinc-400"
        >
          <input
            type="checkbox"
            checked={selected.has(index)}
            onChange={(event) => onToggle(index, event.target.checked)}
            className="accent-blue-500"
          />
          <Badge variant="default">{item.kind}</Badge>
          <span className="font-mono text-zinc-300">{item.id}</span>
          {item.name ? <span className="text-zinc-500">{item.name}</span> : null}
        </label>
      ))}
      {preview.warnings.length > 0 ? (
        <div className="text-[13px] text-amber-300/80">
          {preview.warnings.slice(0, 5).map((warning) => (
            <p key={warning}>⚠ {warning}</p>
          ))}
        </div>
      ) : null}
      <div className="grid gap-2 sm:grid-cols-3">
        <select
          value={scope}
          onChange={(event) => onScopeChange(event.target.value as "workspace" | "global")}
          className={inputClass}
          aria-label="Escopo da instalação"
        >
          <option value="workspace">workspace</option>
          <option value="global">global</option>
        </select>
        {scope === "workspace" ? (
          <select
            value={targetWorkspace}
            onChange={(event) => onTargetWorkspaceChange(event.target.value)}
            className={inputClass}
            aria-label="Workspace de destino"
          >
            {workspaces.map((workspace) => (
              <option key={workspace.id} value={workspace.id}>
                {workspace.name}
              </option>
            ))}
          </select>
        ) : (
          <div />
        )}
        <label className="flex items-center gap-2 text-[13.5px] text-zinc-400">
          <input
            type="checkbox"
            checked={overwrite}
            onChange={(event) => onOverwriteChange(event.target.checked)}
            className="accent-blue-500"
          />
          sobrescrever
        </label>
      </div>
    </div>
  );
}
