import { Loader2, Plus, Search } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { ModelCombobox } from "@/components/ui/model-combobox";
import type { ChatThread } from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { smallLabelClass as labelClass } from "@/lib/ui-classes";

export function ThreadListCard({
  threads,
  threadQuery,
  onThreadQueryChange,
  selectedId,
  onSelect,
  agent,
  onAgentChange,
  model,
  onModelChange,
  workspaceId,
  loading,
  onCreate,
}: {
  threads: ChatThread[];
  threadQuery: string;
  onThreadQueryChange: (value: string) => void;
  selectedId: string | null;
  onSelect: (id: string) => void;
  agent: string;
  onAgentChange: (value: string) => void;
  model: string | null;
  onModelChange: (value: string | null) => void;
  workspaceId: string;
  loading: boolean;
  onCreate: () => void;
}) {
  const filteredThreads = threads.filter((item) => {
    const needle = threadQuery.trim().toLowerCase();
    if (!needle) return true;
    return `${item.title} ${item.agent}`.toLowerCase().includes(needle);
  });

  return (
    <Card className="flex h-full min-h-0 flex-col overflow-hidden">
      <CardHeader className="px-4 py-3.5">
        <div>
          <CardTitle>Conversas</CardTitle>
          <p className="mt-0.5 text-[13.5px] text-zinc-500">
            {threads.length} conversa{threads.length === 1 ? "" : "s"}
          </p>
        </div>
        <Button size="xs" onClick={onCreate} disabled={loading}>
          <Plus size={13} />
          Nova
        </Button>
      </CardHeader>
      <div className="border-b border-zinc-800/70 p-2.5">
        <div className="flex h-9 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5 focus-within:border-zinc-600">
          <Search size={13} className="text-zinc-500" />
          <input
            value={threadQuery}
            onChange={(event) => onThreadQueryChange(event.target.value)}
            placeholder="Buscar conversa ou agente"
            className="h-full flex-1 bg-transparent text-[14px] outline-none placeholder:text-zinc-500"
            aria-label="Buscar conversa ou agente"
          />
        </div>
      </div>
      <CardContent
        tabIndex={0}
        aria-label="Lista de conversas"
        className="min-h-0 flex-1 space-y-1 overflow-y-auto p-2"
      >
        {filteredThreads.length === 0 ? (
          <p className="p-4 text-center text-[13.5px] text-zinc-500">
            {threads.length === 0
              ? "Nenhuma conversa. Escolha um agente e crie a primeira."
              : "Nenhuma conversa encontrada."}
          </p>
        ) : (
          filteredThreads.map((item) => (
            <button
              type="button"
              key={item.id}
              onClick={() => onSelect(item.id)}
              className={cn(
                "w-full rounded-lg px-3 py-2.5 text-left transition-colors",
                selectedId === item.id
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-400 hover:bg-zinc-900",
              )}
            >
              <div className="flex items-center gap-1.5">
                <p className="min-w-0 flex-1 truncate text-[13.5px]">{item.title}</p>
                {item.source !== "desktop" ? (
                  <Badge variant="purple">{item.source}</Badge>
                ) : null}
              </div>
              <p className="mt-0.5 truncate font-mono text-[11.5px] text-zinc-500">
                @{item.agent} · {relativeTime(item.last_message_at ?? item.updated_at)}
              </p>
            </button>
          ))
        )}
      </CardContent>
      <div className="space-y-2 border-t border-zinc-800/70 p-3">
        <div className="block">
          <span className={labelClass}>Agente para novas conversas</span>
          <AgentCombobox
            value={agent}
            onChange={(value) => onAgentChange(value)}
            workspaceId={workspaceId}
            placeholder="build"
            allowEmpty={false}
          />
        </div>
        <div className="block">
          <span className={labelClass}>Modelo (opcional)</span>
          <ModelCombobox
            value={model}
            onChange={onModelChange}
            routerLabel="Automático"
            routerHint="Sem modelo fixo, a conversa usa o modelo padrão do agente no OpenCode; o roteador DSH não interfere."
          />
        </div>
        <Button size="sm" className="w-full" onClick={onCreate} disabled={loading}>
          {loading ? <Loader2 size={13} className="animate-spin" /> : <Plus size={13} />}
          Criar conversa com @{agent}
        </Button>
      </div>
    </Card>
  );
}
