import { useEffect, useMemo, useState } from "react";
import type { ModelHealth, ModelInfo } from "@/lib/types";
import { loadModelHealth, loadModels } from "@/lib/studio-cache";
import { isValidModelRef, normalizeModelRef } from "@/lib/model-ref";
import { toModelSelection } from "@/lib/model-selection";
import { shouldCloseOnBlur } from "@/lib/combobox";
import { useAppStore } from "@/store/app-store";
import { cn } from "@/lib/utils";
import { ModelTestButton } from "@/components/ui/model-test-button";

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 font-mono text-[12.5px] text-zinc-100 outline-none placeholder:text-zinc-500 transition-colors hover:border-zinc-700 focus:border-blue-500/60";

export function ModelCombobox({
  value,
  onChange,
  allowRouter = true,
  routerLabel = "Roteador automático",
  routerHint = "O roteador escolhe o modelo por custo e qualidade. O modelo não muda sem esta opção.",
  placeholder = "digite para filtrar (ex.: deepseek, glm, opencode-go/…)",
  ariaLabel = "Modelo",
  className,
}: {
  value: string | null;
  onChange: (value: string | null) => void;
  allowRouter?: boolean;
  routerLabel?: string;
  routerHint?: string;
  placeholder?: string;
  ariaLabel?: string;
  className?: string;
}) {
  const current = toModelSelection(value);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [health, setHealth] = useState<ModelHealth[]>([]);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState(current ?? "");
  const [editing, setEditing] = useState(!allowRouter || current !== null);
  const [touched, setTouched] = useState(false);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);

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

  useEffect(() => {
    setQuery(current ?? "");
    if (!allowRouter) {
      setEditing(true);
    } else {
      setEditing(current !== null);
    }
  }, [current, allowRouter]);

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
          ...(current ? [current] : []),
        ]),
      ),
    [models, current],
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

  const invalid =
    touched &&
    editing &&
    (query.trim().length === 0 || !isValidModelRef(query));

  function chooseModel(candidate: string) {
    const normalized = normalizeModelRef(candidate) ?? candidate.trim();
    if (!normalized) return;
    onChange(normalized);
    setQuery(normalized);
    setOpen(false);
    setEditing(true);
    setTouched(false);
  }

  function chooseRouter() {
    onChange(null);
    setQuery("");
    setOpen(false);
    setEditing(false);
    setTouched(false);
  }

  function commitInput() {
    const normalized = normalizeModelRef(query);
    if (normalized) {
      onChange(normalized);
      setQuery(normalized);
      return true;
    }
    if (!query.trim() && allowRouter) {
      chooseRouter();
      return true;
    }
    return false;
  }

  return (
    <div className={cn("space-y-1.5", className)}>
      {allowRouter ? (
        <div
          role="group"
          aria-label={ariaLabel}
          className="flex rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5"
        >
          <button
            type="button"
            aria-pressed={!editing}
            onClick={chooseRouter}
            className={cn(
              "flex-1 rounded-md px-2.5 py-1.5 text-[12px] font-medium transition-colors",
              !editing
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-300",
            )}
          >
            {routerLabel}
          </button>
          <button
            type="button"
            aria-pressed={editing}
            onClick={() => {
              setEditing(true);
              setQuery(current ?? "");
              setOpen(true);
            }}
            className={cn(
              "flex-1 rounded-md px-2.5 py-1.5 text-[12px] font-medium transition-colors",
              editing
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-300",
            )}
          >
            Modelo específico
          </button>
        </div>
      ) : null}

      {allowRouter && !editing ? (
        <p className="px-0.5 text-[11.5px] leading-relaxed text-zinc-500">
          {routerHint}
        </p>
      ) : (
        <div className="relative">
          <input
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setOpen(true);
            }}
            onFocus={() => {
              setOpen(true);
              setQuery(current ?? "");
            }}
            onBlur={(event) => {
              if (shouldCloseOnBlur(event)) setOpen(false);
              setTouched(true);
              if (!commitInput() && !query.trim() && !allowRouter) {
                setQuery("");
              }
            }}
            onKeyDown={(event) => {
              if (event.key === "Escape") setOpen(false);
              if (event.key === "Enter") {
                event.preventDefault();
                if (filtered[0]) {
                  chooseModel(filtered[0]);
                } else {
                  commitInput();
                  setOpen(false);
                }
              }
            }}
            placeholder={placeholder}
            className={inputClass}
            aria-label={ariaLabel}
            aria-expanded={open}
            aria-invalid={invalid}
            role="combobox"
          />
          {open ? (
            <div
              role="listbox"
              className="absolute z-40 mt-1 max-h-60 w-full min-w-[320px] overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl"
            >
              {allowRouter ? (
                <button
                  type="button"
                  role="option"
                  aria-selected={current === null}
                  className="block w-full rounded-md px-2.5 py-2 text-left text-[12px] text-zinc-400 hover:bg-zinc-800"
                  onMouseDown={(event) => {
                    event.preventDefault();
                    chooseRouter();
                  }}
                >
                  {routerLabel} — decidir por custo e qualidade
                </button>
              ) : null}
              {filtered.map((candidate) => (
                <button
                  key={candidate}
                  type="button"
                  role="option"
                  aria-selected={candidate === current}
                  className={cn(
                    "block w-full rounded-md px-2.5 py-2 text-left font-mono text-[12px] hover:bg-zinc-800",
                    candidate === current
                      ? "bg-zinc-800 text-zinc-100"
                      : "text-zinc-300",
                  )}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    chooseModel(candidate);
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
                    chooseModel(query);
                  }}
                >
                  usar “{query.trim()}”
                </button>
              ) : null}
              {filtered.length === 0 && !query.trim() ? (
                <p className="px-2 py-1 text-[10px] text-zinc-500">
                  Nenhum modelo disponível. Digite um valor personalizado.
                </p>
              ) : null}
            </div>
          ) : null}
        </div>
      )}

      {invalid || (editing && query.trim() && !isValidModelRef(query)) ? (
        <p className="text-[10px] text-red-300">
          Modelo inválido. Use provider/modelo ou provider/modelo#variant.
        </p>
      ) : null}
      {editing && current && failed(current) ? (
        <p className="text-[9px] text-amber-300">
          Este modelo falhou recentemente. Escolha outro ou limpe o cooldown em
          Configurações → OpenCode Engine.
        </p>
      ) : null}
      {editing && current ? (
        <ModelTestButton model={current} workspaceId={activeWorkspaceId} />
      ) : null}
    </div>
  );
}
