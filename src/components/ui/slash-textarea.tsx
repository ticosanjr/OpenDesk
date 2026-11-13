import { useEffect, useMemo, useRef, useState } from "react";
import type { StudioRuntime } from "@/lib/types";
import { loadAgents, loadRuntime } from "@/lib/studio-cache";
import { cn } from "@/lib/utils";

interface SlashItem {
  value: string;
  label: string;
  detail: string;
}

type Trigger = "@" | "/" | "#";

export function SlashTextarea({
  value,
  onChange,
  workspaceId,
  rows = 5,
  placeholder,
  className,
  disabled,
  onSubmit,
  submitOnEnter = false,
  showHints = true,
}: {
  value: string;
  onChange: (value: string) => void;
  workspaceId?: string | null;
  rows?: number;
  placeholder?: string;
  className?: string;
  disabled?: boolean;
  onSubmit?: () => void;
  submitOnEnter?: boolean;
  showHints?: boolean;
}) {
  const ref = useRef<HTMLTextAreaElement | null>(null);
  const [runtime, setRuntime] = useState<StudioRuntime | null>(null);
  const [agents, setAgents] = useState<Array<{ id: string; name?: string | null; description?: string | null }>>([]);
  const [slash, setSlash] = useState<{ trigger: Trigger; query: string; start: number } | null>(null);
  const [active, setActive] = useState(0);

  useEffect(() => {
    let cancelled = false;
    void loadRuntime(workspaceId).then((value) => {
      if (!cancelled) setRuntime(value);
    });
    void loadAgents(workspaceId).then((value) => {
      if (!cancelled) setAgents(value);
    });
    return () => {
      cancelled = true;
    };
  }, [workspaceId]);

  const items = useMemo<SlashItem[]>(() => {
    if (!slash) return [];
    const query = slash.query.toLowerCase();
    if (slash.trigger === "@") {
      return agents
        .filter((agent) => `${agent.id} ${agent.name ?? ""}`.toLowerCase().includes(query))
        .slice(0, 14)
        .map((agent) => ({
          value: `@${agent.id}`,
          label: `@${agent.id}`,
          detail: agent.name || agent.description || "agente",
        }));
    }
    if (slash.trigger === "#") {
      return (runtime?.skills ?? [])
        .filter((skill) => skill.id.toLowerCase().includes(query))
        .slice(0, 14)
        .map((skill) => ({
          value: `#${skill.id}`,
          label: `#${skill.id}`,
          detail: skill.description ?? "skill",
        }));
    }
    return (runtime?.commands ?? [])
      .filter((command) => command.name.toLowerCase().includes(query))
      .slice(0, 14)
      .map((command) => ({
        value: `/${command.name}`,
        label: `/${command.name}`,
        detail: command.description ?? "comando",
      }));
  }, [agents, runtime, slash]);

  function update(next: string, cursor: number) {
    onChange(next);
    const before = next.slice(0, cursor);
    const match =
      before.match(/(?:^|\s)([@/])([\w:.-]*)$/) ??
      before.match(/(?:^|\s)(#)([\w:.-]+)$/);
    if (match) {
      setSlash({
        trigger: match[1] as Trigger,
        query: match[2],
        start: cursor - match[2].length - 1,
      });
      setActive(0);
    } else {
      setSlash(null);
    }
  }

  function insert(item: SlashItem) {
    if (!slash) return;
    const before = value.slice(0, slash.start);
    const after = value.slice(slash.start + slash.query.length + 1);
    const next = `${before}${item.value} ${after}`;
    onChange(next);
    setSlash(null);
    window.setTimeout(() => ref.current?.focus(), 0);
  }

  return (
    <div className="relative">
      <textarea
        ref={ref}
        value={value}
        disabled={disabled}
        onChange={(event) =>
          update(event.target.value, event.target.selectionStart ?? 0)
        }
        onKeyDown={(event) => {
          if (slash) {
            if (event.key === "Escape") {
              event.preventDefault();
              setSlash(null);
              return;
            }
            if (event.key === "ArrowDown") {
              event.preventDefault();
              setActive((current) =>
                items.length === 0 ? 0 : (current + 1) % items.length,
              );
              return;
            }
            if (event.key === "ArrowUp") {
              event.preventDefault();
              setActive((current) =>
                items.length === 0
                  ? 0
                  : (current - 1 + items.length) % items.length,
              );
              return;
            }
            if (event.key === "Enter" && !event.shiftKey && items.length > 0) {
              event.preventDefault();
              insert(items[Math.min(active, items.length - 1)]);
              return;
            }
          }
          if (
            event.key === "Enter" &&
            (event.metaKey || event.ctrlKey || (submitOnEnter && !event.shiftKey))
          ) {
            event.preventDefault();
            if (onSubmit) {
              onSubmit();
            } else {
              const form = event.currentTarget.closest("form");
              form?.requestSubmit();
            }
          }
        }}
        rows={rows}
        placeholder={placeholder}
        className={cn(
          "w-full resize-y rounded-lg border border-zinc-800 bg-zinc-950/50 px-3 py-2 text-[13px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60 disabled:opacity-60",
          className,
        )}
        aria-label="Comando ou prompt"
      />
      {showHints ? (
        <p className="mt-1 text-[11.5px] leading-snug text-zinc-500">
          Use{" "}
          <span className="font-mono text-zinc-400">@agente</span> para chamar um
          agente,{" "}
          <span className="font-mono text-zinc-400">/comando</span> para um comando
          e{" "}
          <span className="font-mono text-zinc-400">#skill</span> para uma skill.
        </p>
      ) : null}
      {slash ? (
        <div
          role="listbox"
          className="absolute z-40 mt-1 max-h-64 w-full overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl"
        >
          {items.length === 0 ? (
            <p className="px-2 py-1 text-[12px] text-zinc-500">
              Nenhum comando/skill encontrado no runtime.
            </p>
          ) : (
            items.map((item, index) => (
              <button
                key={`${item.label}:${item.detail}`}
                type="button"
                role="option"
                aria-selected={index === active}
                className={cn(
                  "block w-full rounded px-2 py-1.5 text-left hover:bg-zinc-800",
                  index === active && "bg-zinc-800/80",
                )}
                onMouseDown={(event) => {
                  event.preventDefault();
                  insert(item);
                }}
              >
                <span className="font-mono text-[12px] text-zinc-200">
                  {item.label}
                </span>
                <span className="ml-2 text-[11.5px] text-zinc-500">{item.detail}</span>
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}
