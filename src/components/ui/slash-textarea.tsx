import { useEffect, useMemo, useRef, useState } from "react";
import type { StudioRuntime } from "@/lib/types";
import { loadRuntime } from "@/lib/studio-cache";
import { cn } from "@/lib/utils";

interface SlashItem {
  value: string;
  label: string;
  detail: string;
}

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
}) {
  const ref = useRef<HTMLTextAreaElement | null>(null);
  const [runtime, setRuntime] = useState<StudioRuntime | null>(null);
  const [slash, setSlash] = useState<{ query: string; start: number } | null>(null);
  const [active, setActive] = useState(0);

  useEffect(() => {
    let cancelled = false;
    void loadRuntime(workspaceId).then((value) => {
      if (!cancelled) setRuntime(value);
    });
    return () => {
      cancelled = true;
    };
  }, [workspaceId]);

  const items = useMemo<SlashItem[]>(() => {
    if (!slash) return [];
    const query = slash.query.toLowerCase();
    const commands = (runtime?.commands ?? [])
      .filter((command) => command.name.toLowerCase().includes(query))
      .slice(0, 12)
      .map((command) => ({
        value: `/${command.name}`,
        label: `/${command.name}`,
        detail: command.description ?? "comando",
      }));
    const skills = (runtime?.skills ?? [])
      .filter((skill) => skill.id.toLowerCase().includes(query))
      .slice(0, 12)
      .map((skill) => ({
        value: `skill:${skill.id}`,
        label: `skill: ${skill.id}`,
        detail: skill.description ?? "skill",
      }));
    return [...commands, ...skills];
  }, [runtime, slash]);

  function update(next: string, cursor: number) {
    onChange(next);
    const before = next.slice(0, cursor);
    const match = before.match(/(?:^|\s)\/([\w:.-]*)$/);
    if (match) {
      setSlash({ query: match[1], start: cursor - match[1].length - 1 });
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
          "w-full resize-y rounded-lg border border-zinc-800 bg-zinc-950/50 px-3 py-2 text-[11px] text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-blue-500/60 disabled:opacity-60",
          className,
        )}
        aria-label="Comando ou prompt"
      />
      {slash ? (
        <div
          role="listbox"
          className="absolute z-40 mt-1 max-h-64 w-full overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl"
        >
          {items.length === 0 ? (
            <p className="px-2 py-1 text-[10px] text-zinc-600">
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
                <span className="font-mono text-[10px] text-zinc-200">
                  {item.label}
                </span>
                <span className="ml-2 text-[9px] text-zinc-600">{item.detail}</span>
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}
