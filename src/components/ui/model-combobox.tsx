import { useEffect, useMemo, useState } from "react";
import type { ModelHealth, ModelInfo } from "@/lib/types";
import { loadModelHealth, loadModels } from "@/lib/studio-cache";
import { cn } from "@/lib/utils";

export function ModelCombobox({
  value,
  onChange,
  placeholder = "digite para filtrar (ex.: deepseek, glm, opencode-go/…)",
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
}) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [health, setHealth] = useState<ModelHealth[]>([]);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");

  useEffect(() => {
    let cancelled = false;
    void Promise.all([loadModels(), loadModelHealth()]).then(([list, healthList]) => {
      if (cancelled) return;
      setModels(list);
      setHealth(healthList);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const values = useMemo(
    () =>
      Array.from(
        new Set([
          ...models
            .map((item) => {
              const provider = item.providerID ?? "";
              const id = item.modelID ?? item.id;
              return `${provider}/${id}`;
            })
            .filter(Boolean),
          ...(value ? [value] : []),
        ]),
      ),
    [models, value],
  );

  const failed = (candidate: string) => {
    const base = candidate.split("#")[0];
    return health.some(
      (item) => `${item.provider_id}/${item.model_id}` === base,
    );
  };

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return values;
    return values.filter((candidate) => candidate.toLowerCase().includes(needle));
  }, [query, values]);

  return (
    <div className={cn("relative", className)}>
      <input
        value={open ? query : value}
        onChange={(event) => {
          setQuery(event.target.value);
          onChange(event.target.value);
          setOpen(true);
        }}
        onFocus={() => {
          setQuery(value);
          setOpen(true);
        }}
        onBlur={() => window.setTimeout(() => setOpen(false), 140)}
        onKeyDown={(event) => {
          if (event.key === "Escape") setOpen(false);
          if (event.key === "Enter" && filtered[0]) {
            event.preventDefault();
            onChange(filtered[0]);
            setOpen(false);
          }
        }}
        placeholder={placeholder}
        className={inputClass}
        aria-label="Modelo"
        aria-expanded={open}
        role="combobox"
      />
      {open ? (
        <div
          role="listbox"
          className="absolute z-40 mt-1 max-h-60 w-full min-w-[320px] overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl"
        >
          <button
            type="button"
            role="option"
            aria-selected={!value}
            className="block w-full rounded px-2 py-1.5 text-left text-[10px] text-zinc-400 hover:bg-zinc-800"
            onMouseDown={(event) => {
              event.preventDefault();
              onChange("");
              setOpen(false);
            }}
          >
            {placeholder}
          </button>
          {filtered.map((candidate) => (
            <button
              key={candidate}
              type="button"
              role="option"
              aria-selected={candidate === value}
              className={cn(
                "block w-full rounded px-2 py-1.5 text-left font-mono text-[10px] hover:bg-zinc-800",
                candidate === value ? "bg-zinc-800 text-zinc-100" : "text-zinc-300",
              )}
              onMouseDown={(event) => {
                event.preventDefault();
                onChange(candidate);
                setOpen(false);
              }}
            >
              {candidate}
              {failed(candidate) ? (
                <span className="ml-1 text-amber-300">⚠ falhou recentemente</span>
              ) : null}
            </button>
          ))}
          {query.trim() && !values.includes(query.trim()) ? (
            <button
              type="button"
              className="block w-full rounded px-2 py-1.5 text-left font-mono text-[10px] text-blue-300 hover:bg-zinc-800"
              onMouseDown={(event) => {
                event.preventDefault();
                onChange(query.trim());
                setOpen(false);
              }}
            >
              usar “{query.trim()}”
            </button>
          ) : null}
          {filtered.length === 0 && !query.trim() ? (
            <p className="px-2 py-1 text-[10px] text-zinc-600">
              Nenhum modelo disponível. Digite um valor personalizado.
            </p>
          ) : null}
        </div>
      ) : null}
      {value && failed(value) ? (
        <p className="mt-1 text-[9px] text-amber-300">
          Este modelo falhou recentemente. Escolha outro ou limpe o cooldown em
          Configurações → OpenCode Engine.
        </p>
      ) : null}
    </div>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 font-mono text-[10px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60";
