import { useEffect, useMemo, useState } from "react";
import type { AgentFile } from "@/lib/types";
import { loadAgents, loadRuntime } from "@/lib/studio-cache";
import { cn } from "@/lib/utils";

const BUILTIN_AGENTS = ["build", "plan", "explore", "general"];

interface AgentOption {
  id: string;
  name?: string | null;
  description?: string | null;
  router_policy?: string | null;
  runtime?: boolean;
}

export function AgentCombobox({
  value,
  onChange,
  workspaceId,
  placeholder = "automático (router)",
  className,
  allowEmpty = true,
}: {
  value: string;
  onChange: (value: string, agent: AgentFile | null) => void;
  workspaceId?: string | null;
  placeholder?: string;
  className?: string;
  allowEmpty?: boolean;
}) {
  const [agents, setAgents] = useState<AgentFile[]>([]);
  const [runtimeIds, setRuntimeIds] = useState<string[]>([]);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");

  useEffect(() => {
    let cancelled = false;
    void loadAgents(workspaceId).then((list) => {
      if (!cancelled) setAgents(list);
    });
    void loadRuntime(workspaceId).then((runtime) => {
      if (cancelled || !runtime || !Array.isArray(runtime.agents)) return;
      setRuntimeIds(runtime.agents.map((agent) => agent.id));
    });
    return () => {
      cancelled = true;
    };
  }, [workspaceId]);

  const options = useMemo<AgentOption[]>(() => {
    const byId = new Map<string, AgentOption>();
    for (const agent of agents) {
      byId.set(agent.id, {
        id: agent.id,
        name: agent.name,
        description: agent.description,
        router_policy: agent.router_policy,
      });
    }
    for (const id of runtimeIds) {
      if (!byId.has(id)) {
        byId.set(id, { id, name: "runtime", runtime: true });
      }
    }
    for (const id of BUILTIN_AGENTS) {
      if (!byId.has(id)) byId.set(id, { id, name: "built-in", runtime: true });
    }
    if (value && !byId.has(value)) byId.set(value, { id: value, name: "manual" });
    return Array.from(byId.values()).sort((left, right) => left.id.localeCompare(right.id));
  }, [agents, runtimeIds, value]);

  const selected = options.find((agent) => agent.id === value) ?? null;
  const display = value
    ? selected?.name && selected.name !== "manual"
      ? `${selected.name} (@${value})`
      : `@${value}`
    : "";

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return options;
    return options.filter((agent) =>
      `${agent.id} ${agent.name ?? ""} ${agent.description ?? ""}`
        .toLowerCase()
        .includes(needle),
    );
  }, [options, query]);

  function setValue(id: string) {
    const disk = agents.find((agent) => agent.id === id) ?? null;
    onChange(id, disk);
  }

  function choose(agent: AgentOption | null) {
    setValue(agent?.id ?? "");
    setOpen(false);
    setQuery("");
  }

  return (
    <div className={cn("relative", className)}>
      <input
        value={open ? query : display}
        onChange={(event) => {
          setQuery(event.target.value);
          setOpen(true);
        }}
        onFocus={() => {
          setQuery("");
          setOpen(true);
        }}
        onBlur={() => window.setTimeout(() => setOpen(false), 140)}
        onKeyDown={(event) => {
          if (event.key === "Escape") setOpen(false);
          if (event.key === "Enter" && open) {
            event.preventDefault();
            const typed = query.trim();
            const exact = options.find((option) => option.id === typed);
            if (exact) {
              choose(exact);
            } else if (filtered[0]) {
              choose(filtered[0]);
            } else if (typed) {
              choose({ id: typed });
            }
          }
        }}
        placeholder={placeholder}
        className={inputClass}
        aria-label="Agente"
        aria-expanded={open}
        role="combobox"
      />
      {open ? (
        <div
          role="listbox"
          className="absolute z-40 mt-1 max-h-60 w-full min-w-[260px] overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl"
        >
          {allowEmpty ? (
            <button
              type="button"
              role="option"
              aria-selected={!value}
              className="block w-full rounded px-2 py-1.5 text-left text-[10px] text-zinc-400 hover:bg-zinc-800"
              onMouseDown={(event) => {
                event.preventDefault();
                choose(null);
              }}
            >
              {placeholder}
            </button>
          ) : null}
          {filtered.map((agent) => (
            <button
              key={agent.id}
              type="button"
              role="option"
              aria-selected={agent.id === value}
              className={cn(
                "block w-full rounded px-2 py-1.5 text-left hover:bg-zinc-800",
                agent.id === value && "bg-zinc-800/70",
              )}
              onMouseDown={(event) => {
                event.preventDefault();
                choose(agent);
              }}
            >
              <span className="font-mono text-[10px] text-zinc-200">@{agent.id}</span>
              {agent.name ? (
                <span className="ml-1 text-[10px] text-zinc-400">{agent.name}</span>
              ) : null}
              {agent.router_policy ? (
                <span className="ml-1 font-mono text-[9px] text-zinc-600">
                  {agent.router_policy}
                </span>
              ) : null}
              {agent.description ? (
                <span className="mt-0.5 block truncate text-[9px] text-zinc-600">
                  {agent.description}
                </span>
              ) : null}
            </button>
          ))}
          {query.trim() && !filtered.some((agent) => agent.id === query.trim()) ? (
            <button
              type="button"
              role="option"
              className="block w-full rounded px-2 py-1.5 text-left font-mono text-[10px] text-blue-300 hover:bg-zinc-800"
              onMouseDown={(event) => {
                event.preventDefault();
                choose({ id: query.trim() });
              }}
            >
              usar “{query.trim()}”
            </button>
          ) : null}
          {filtered.length === 0 ? (
            <p className="px-2 py-1 text-[10px] text-zinc-600">
              Nenhum agente encontrado. Digite um ID para usar manualmente.
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60";
