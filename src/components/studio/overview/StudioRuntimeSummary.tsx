import { Badge } from "@/components/ui/badge";
import type { EffectiveStudioItem, StudioRuntime } from "@/lib/types";

export function StudioRuntimeSummary({
  items,
  runtime,
  onImportRuntime,
}: {
  items: EffectiveStudioItem[];
  runtime: StudioRuntime | null;
  onImportRuntime: (kind: "skill" | "agent", id: string) => void;
}) {
  return (
    <>
      <div className="mb-3 grid gap-2 lg:grid-cols-3">
        <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
          <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">
            Inventário em disco
          </p>
          <p className="mt-1 text-[13px] text-zinc-300">
            {items.filter((item) => item.source_scope === "global").length} globais ·{" "}
            {items.filter((item) => item.source_scope === "workspace").length} no workspace ·{" "}
            {items.filter((item) => item.disabled).length} desativados
          </p>
        </div>
        <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
          <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">
            Agentes runtime do OpenCode
          </p>
          <p className="mt-1 truncate text-[13px] text-zinc-300">
            {runtime?.engine.online
              ? `${runtime.agents.length} disponíveis · ${runtime.agents
                  .slice(0, 6)
                  .map((agent) => `@${agent.id}`)
                  .join(" ")}`
              : "engine offline — inicie o motor para listar agentes runtime"}
          </p>
        </div>
        <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
          <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">
            Modelos disponíveis
          </p>
          <p className="mt-1 text-[13px] text-zinc-300">
            {runtime?.engine.online
              ? `${runtime.models.length} modelos reportados pelo OpenCode`
              : "engine offline"}
          </p>
        </div>
      </div>
      {runtime?.engine.online && runtime.agents.length > 0 ? (
        <div className="mb-3 flex flex-wrap gap-1.5">
          {runtime.agents.map((agent) => (
            <Badge key={agent.id} variant="default">
              @{agent.id}
            </Badge>
          ))}
        </div>
      ) : null}
      {runtime?.engine.online ? (
        <div className="mb-3 grid gap-2 lg:grid-cols-3">
          <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
            <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">
              Skills carregadas pelo OpenCode
            </p>
            <p className="mt-1 text-[13px] text-zinc-300">
              {runtime.skills.length} disponíveis
            </p>
            <div className="mt-1 flex flex-wrap gap-1">
              {runtime.skills.slice(0, 12).map((skill) => (
                <Badge key={skill.id} variant="purple">
                  {skill.id}
                </Badge>
              ))}
              {runtime.skills.length > 12 ? (
                <Badge variant="default">+{runtime.skills.length - 12}</Badge>
              ) : null}
            </div>
            {runtime.skills.length > 0 ? (
              <details className="mt-2">
                <summary className="cursor-pointer text-[11.5px] text-zinc-500">
                  ver todas
                </summary>
                <div className="mt-1 max-h-40 overflow-y-auto text-[11.5px] text-zinc-500">
                  {runtime.skills.map((skill) => (
                    <div key={skill.id} className="flex items-center gap-1">
                      <p className="min-w-0 flex-1 truncate">
                        <span className="font-mono text-zinc-300">{skill.id}</span>
                        {skill.description ? ` · ${skill.description}` : ""}
                      </p>
                      <button
                        type="button"
                        className="shrink-0 text-[11px] text-blue-300/80 underline-offset-2 hover:underline"
                        onClick={() => onImportRuntime("skill", skill.id)}
                      >
                        importar
                      </button>
                    </div>
                  ))}
                </div>
              </details>
            ) : null}
          </div>
          <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
            <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">
              Comandos carregados pelo OpenCode
            </p>
            <p className="mt-1 text-[13px] text-zinc-300">
              {runtime.commands.length} disponíveis
            </p>
            <div className="mt-1 max-h-40 overflow-y-auto text-[11.5px] text-zinc-500">
              {runtime.commands.map((command) => (
                <p key={command.name} className="truncate">
                  <span className="font-mono text-zinc-300">/{command.name}</span>
                  {command.description ? ` · ${command.description}` : ""}
                </p>
              ))}
            </div>
          </div>
          <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
            <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">
              MCPs carregados pelo OpenCode
            </p>
            <p className="mt-1 text-[13px] text-zinc-300">
              {runtime.mcps.length} configurados
            </p>
            <div className="mt-1 space-y-1">
              {runtime.mcps.map((mcp) => {
                const status =
                  typeof mcp.status === "string"
                    ? mcp.status
                    : mcp.status?.status ?? "unknown";
                return (
                  <div key={mcp.name} className="flex items-center gap-2 text-[11.5px]">
                    <span className="font-mono text-zinc-300">{mcp.name}</span>
                    <Badge variant={status === "connected" ? "green" : "red"}>
                      {status}
                    </Badge>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
