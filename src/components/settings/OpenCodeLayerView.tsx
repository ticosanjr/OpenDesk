import { Sparkles } from "lucide-react";
import type { PromptPreview } from "@/lib/types";

export function OpenCodeLayerView({
  layer,
}: {
  layer: PromptPreview["opencode"];
}) {
  return (
    <div className="space-y-2">
      <p className="rounded-lg border border-amber-500/20 bg-amber-500/[0.04] px-3 py-2 text-[11.5px] text-amber-200/80">
        {layer.note ??
          "Este conteúdo é carregado pelo OpenCode, não faz parte do prompt enviado pelo OpenDesk."}
      </p>
      <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
        <p className="flex items-center gap-1.5 text-[12.5px] font-medium text-zinc-300">
          <Sparkles size={12} />
          Agente
        </p>
        {layer.agent ? (
          <>
            <p className="mt-1 font-mono text-[11.5px] text-zinc-500">
              {layer.agent.scope} · {layer.agent.path}
            </p>
            <pre className="mt-2 max-h-72 overflow-auto whitespace-pre-wrap font-mono text-[11.5px] leading-relaxed text-zinc-400">
              {layer.agent.system || "(sem prompt próprio)"}
            </pre>
          </>
        ) : (
          <p className="mt-1 text-[12px] text-zinc-500">
            Agente sem arquivo de prompt próprio.
          </p>
        )}
      </div>
      {layer.agents_md.map((file) => (
        <div
          key={`${file.scope}:${file.path}`}
          className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3"
        >
          <p className="text-[12.5px] font-medium text-zinc-300">
            AGENTS.md · {file.scope}
          </p>
          <p className="mt-1 break-all font-mono text-[11.5px] text-zinc-500">
            {file.path}
          </p>
          <pre className="mt-2 max-h-56 overflow-auto whitespace-pre-wrap font-mono text-[11.5px] leading-relaxed text-zinc-400">
            {file.content}
          </pre>
        </div>
      ))}
      <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
        <p className="text-[12.5px] font-medium text-zinc-300">
          Skills disponíveis
        </p>
        {layer.skills.length === 0 ? (
          <p className="mt-1 text-[12px] text-zinc-500">Nenhuma skill.</p>
        ) : (
          <div className="mt-1 space-y-1">
            {layer.skills.map((skill) => (
              <p
                key={`${skill.scope}:${skill.id}`}
                className="text-[12px] text-zinc-400"
              >
                <span className="font-mono text-zinc-500">{skill.scope}</span>{" "}
                {skill.name}
                {skill.description ? ` — ${skill.description}` : ""}
              </p>
            ))}
          </div>
        )}
      </div>
      <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
        <p className="text-[12.5px] font-medium text-zinc-300">Comandos</p>
        {layer.commands.length === 0 ? (
          <p className="mt-1 text-[12px] text-zinc-500">Nenhum comando.</p>
        ) : (
          <div className="mt-1 space-y-1">
            {layer.commands.map((command) => (
              <p
                key={`${command.scope}:${command.id}`}
                className="text-[12px] text-zinc-400"
              >
                <span className="font-mono text-zinc-500">{command.scope}</span>{" "}
                /{command.id}
                {command.description ? ` — ${command.description}` : ""}
              </p>
            ))}
          </div>
        )}
      </div>
      <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
        <p className="text-[12.5px] font-medium text-zinc-300">Servidores MCP</p>
        {layer.mcp_servers.length === 0 ? (
          <p className="mt-1 text-[12px] text-zinc-500">Nenhum servidor MCP.</p>
        ) : (
          <pre className="mt-1 max-h-56 overflow-auto whitespace-pre-wrap font-mono text-[11.5px] leading-relaxed text-zinc-400">
            {JSON.stringify(
              layer.mcp_servers.map((item) => ({
                scope: item.scope,
                ...(typeof item.server === "object" && item.server
                  ? item.server
                  : { server: item.server }),
              })),
              null,
              2,
            )}
          </pre>
        )}
      </div>
    </div>
  );
}
