import { useMemo, useState } from "react";
import {
  Check,
  ChevronDown,
  ChevronRight,
  ChevronsDownUp,
  ChevronsUpDown,
  Copy,
  Search,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const MAX_TREE_NODES = 5000;
const MAX_MATCHES = 200;

type JsonValue = unknown;

function isPlainObject(value: JsonValue): value is Record<string, JsonValue> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isContainer(value: JsonValue): boolean {
  return Array.isArray(value) || isPlainObject(value);
}

function childEntries(value: JsonValue): Array<[string, JsonValue]> {
  if (Array.isArray(value)) return value.map((item, index) => [String(index), item]);
  if (isPlainObject(value)) return Object.entries(value);
  return [];
}

function childPath(path: string, key: string): string {
  return /^\d+$/.test(key) ? `${path}[${key}]` : `${path}.${key}`;
}

function preview(value: JsonValue): string {
  if (Array.isArray(value)) return `[ ${value.length} ${value.length === 1 ? "item" : "itens"} ]`;
  if (isPlainObject(value)) {
    const keys = Object.keys(value);
    return `{ ${keys.length} ${keys.length === 1 ? "chave" : "chaves"} }`;
  }
  if (typeof value === "string") {
    const text = value.length > 60 ? `${value.slice(0, 60)}…` : value;
    return JSON.stringify(text);
  }
  return String(value);
}

function scalarClass(value: JsonValue): string {
  if (value === null) return "text-zinc-500";
  if (typeof value === "string") return "text-emerald-300";
  if (typeof value === "number") return "text-amber-300";
  if (typeof value === "boolean") return "text-blue-300";
  return "text-zinc-300";
}

function countNodes(value: JsonValue, cap = MAX_TREE_NODES + 1): number {
  let total = 1;
  const stack: JsonValue[] = [value];
  while (stack.length > 0 && total < cap) {
    const current = stack.pop();
    for (const [, child] of childEntries(current)) {
      total += 1;
      if (isContainer(child)) stack.push(child);
    }
  }
  return total;
}

function flattenMatches(
  value: JsonValue,
  query: string,
  path = "$",
  found: Array<{ path: string; value: JsonValue }> = [],
): Array<{ path: string; value: JsonValue }> {
  if (found.length >= MAX_MATCHES) return found;
  const needle = query.toLowerCase();
  for (const [key, child] of childEntries(value)) {
    const next = childPath(path, key);
    if (
      key.toLowerCase().includes(needle) ||
      (!isContainer(child) && preview(child).toLowerCase().includes(needle))
    ) {
      found.push({ path: next, value: child });
      if (found.length >= MAX_MATCHES) return found;
    }
    if (isContainer(child)) flattenMatches(child, query, next, found);
    if (found.length >= MAX_MATCHES) return found;
  }
  return found;
}

function parseJson(name: string, content: string): { data: JsonValue; error: string | null } {
  const trimmed = content.trim();
  if (!trimmed) return { data: null, error: "Arquivo vazio." };
  try {
    return { data: JSON.parse(trimmed), error: null };
  } catch {
    if (/\.jsonl$/i.test(name)) {
      const lines = trimmed.split(/\r?\n/).filter((line) => line.trim() !== "");
      try {
        return { data: lines.map((line) => JSON.parse(line)), error: null };
      } catch {
        return { data: null, error: "JSON Lines inválido." };
      }
    }
    return { data: null, error: "JSON inválido ou com comentários." };
  }
}

function Highlight({ text, query }: { text: string; query: string }) {
  if (!query) return <>{text}</>;
  const index = text.toLowerCase().indexOf(query.toLowerCase());
  if (index < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, index)}
      <mark className="rounded bg-amber-400/30 px-0.5 text-amber-100">
        {text.slice(index, index + query.length)}
      </mark>
      {text.slice(index + query.length)}
    </>
  );
}

function JsonNode({
  label,
  value,
  path,
  depth,
  expanded,
  onToggle,
  onCopy,
  copiedPath,
  query,
}: {
  label: string;
  value: JsonValue;
  path: string;
  depth: number;
  expanded: Set<string>;
  onToggle: (path: string) => void;
  onCopy: (path: string, value: JsonValue) => void;
  copiedPath: string | null;
  query: string;
}) {
  const container = isContainer(value);
  const open = container && expanded.has(path);
  return (
    <div>
      <div
        className={cn(
          "group flex items-start gap-1.5 rounded px-1 py-0.5 font-mono text-[12.5px] leading-relaxed hover:bg-zinc-900/60",
          container && "cursor-pointer",
        )}
        style={{ paddingLeft: `${depth * 14 + 4}px` }}
        onClick={container ? () => onToggle(path) : undefined}
      >
        {container ? (
          open ? (
            <ChevronDown size={12} className="mt-1 shrink-0 text-zinc-500" />
          ) : (
            <ChevronRight size={12} className="mt-1 shrink-0 text-zinc-500" />
          )
        ) : (
          <span className="w-3 shrink-0" />
        )}
        <span className="shrink-0 text-sky-300">
          <Highlight text={label} query={query} />
        </span>
        <span className="text-zinc-600">:</span>
        {container ? (
          <span className="text-zinc-500">
            {preview(value)}
            {open ? null : <span className="ml-1 text-zinc-600">…</span>}
          </span>
        ) : (
          <span className={cn("break-all", scalarClass(value))}>
            <Highlight text={preview(value)} query={query} />
          </span>
        )}
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onCopy(path, value);
          }}
          title={`Copiar ${path}`}
          aria-label={`Copiar ${path}`}
          className="ml-auto shrink-0 rounded p-0.5 text-zinc-600 opacity-0 hover:text-zinc-200 focus-visible:opacity-100 group-hover:opacity-100"
        >
          {copiedPath === path ? (
            <Check size={11} className="text-emerald-400" />
          ) : (
            <Copy size={11} />
          )}
        </button>
      </div>
      {open
        ? childEntries(value).map(([key, child]) => (
            <JsonNode
              key={childPath(path, key)}
              label={key}
              value={child}
              path={childPath(path, key)}
              depth={depth + 1}
              expanded={expanded}
              onToggle={onToggle}
              onCopy={onCopy}
              copiedPath={copiedPath}
              query={query}
            />
          ))
        : null}
    </div>
  );
}

export function JsonTreeView({ name, content }: { name: string; content: string }) {
  const parsed = useMemo(() => parseJson(name, content), [content, name]);
  const [view, setView] = useState<"tree" | "code">("tree");
  const [query, setQuery] = useState("");
  const [forceTree, setForceTree] = useState(false);
  const [copied, setCopied] = useState(false);
  const [copiedPath, setCopiedPath] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set(["$"]));

  const nodeCount = useMemo(
    () => (parsed.error ? 0 : countNodes(parsed.data)),
    [parsed],
  );
  const large = nodeCount > MAX_TREE_NODES;
  const matches = useMemo(
    () =>
      parsed.error || query.trim() === ""
        ? []
        : flattenMatches(parsed.data, query.trim()),
    [parsed, query],
  );
  const formatted = useMemo(
    () => (parsed.error ? content : JSON.stringify(parsed.data, null, 2)),
    [content, parsed],
  );

  function toggle(path: string) {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  function expandAll() {
    const next = new Set<string>(["$"]);
    function walk(value: JsonValue, path: string) {
      for (const [key, child] of childEntries(value)) {
        if (!isContainer(child)) continue;
        const nextPath = childPath(path, key);
        next.add(nextPath);
        walk(child, nextPath);
      }
    }
    if (!parsed.error) walk(parsed.data, "$");
    setExpanded(next);
  }

  async function copy(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  }

  async function copyNode(path: string, value: JsonValue) {
    try {
      await navigator.clipboard.writeText(JSON.stringify(value, null, 2));
      setCopiedPath(path);
      window.setTimeout(() => setCopiedPath(null), 1500);
    } catch {
      setCopiedPath(null);
    }
  }

  const effectiveView = large && !forceTree ? "code" : view;

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="flex items-center gap-1 rounded-md border border-zinc-800 p-0.5">
          {(["tree", "code"] as const).map((value) => (
            <button
              key={value}
              type="button"
              onClick={() => setView(value)}
              className={cn(
                "rounded px-1.5 py-0.5 text-[12px]",
                effectiveView === value
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:text-zinc-200",
              )}
            >
              {value === "tree" ? "Árvore" : "Código"}
            </button>
          ))}
        </span>
        <label className="relative min-w-[160px] flex-1">
          <Search
            size={12}
            className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-zinc-500"
          />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Buscar chave ou valor"
            aria-label="Buscar no JSON"
            className="h-7 w-full rounded-md border border-zinc-800 bg-zinc-900/40 pl-7 pr-2 font-mono text-[12px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
          />
        </label>
        {query.trim() !== "" ? (
          <Badge variant="default">
            {matches.length}
            {matches.length >= MAX_MATCHES ? "+" : ""} resultado(s)
          </Badge>
        ) : null}
        {effectiveView === "tree" && query.trim() === "" ? (
          <>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-[11.5px]"
              onClick={expandAll}
              disabled={parsed.error !== null || large}
            >
              <ChevronsUpDown size={12} />
              Expandir tudo
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-[11.5px]"
              onClick={() => setExpanded(new Set(["$"]))}
              disabled={parsed.error !== null}
            >
              <ChevronsDownUp size={12} />
              Recolher tudo
            </Button>
          </>
        ) : null}
        <Button
          variant="ghost"
          size="sm"
          className="ml-auto h-7 px-2 text-[11.5px]"
          onClick={() => void copy(formatted)}
          title="Copiar JSON formatado"
        >
          {copied ? <Check size={12} className="text-emerald-400" /> : <Copy size={12} />}
          {copied ? "copiado" : "copiar JSON"}
        </Button>
      </div>

      {parsed.error ? (
        <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-200">
          {parsed.error} Exibindo o texto original.
        </p>
      ) : null}

      {large ? (
        <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-200">
          JSON com {nodeCount} nós; a árvore foi limitada a {MAX_TREE_NODES}.{" "}
          <button
            type="button"
            className="underline underline-offset-2 hover:text-amber-100"
            onClick={() => setForceTree(true)}
          >
            Renderizar a árvore mesmo assim
          </button>
          .
        </p>
      ) : null}

      {effectiveView === "tree" && !parsed.error ? (
        query.trim() !== "" ? (
          <div className="scroll-area max-h-[60vh] overflow-auto rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-1">
            {matches.length === 0 ? (
              <p className="px-2 py-3 text-[12.5px] text-zinc-500">
                Nenhuma chave ou valor corresponde à busca.
              </p>
            ) : (
              matches.map((match) => (
                <button
                  key={match.path}
                  type="button"
                  onClick={() => void copy(match.path)}
                  title="Copiar o caminho"
                  className="flex w-full items-start gap-2 rounded px-1.5 py-1 text-left font-mono text-[12px] hover:bg-zinc-900/70"
                >
                  <span className="shrink-0 text-sky-300">
                    <Highlight text={match.path} query={query.trim()} />
                  </span>
                  <span className={cn("truncate", scalarClass(match.value))}>
                    {isContainer(match.value) ? (
                      preview(match.value)
                    ) : (
                      <Highlight text={preview(match.value)} query={query.trim()} />
                    )}
                  </span>
                </button>
              ))
            )}
          </div>
        ) : (
          <div className="scroll-area max-h-[60vh] overflow-auto rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-1">
            <JsonNode
              label="$"
              value={parsed.data}
              path="$"
              depth={0}
              expanded={expanded}
              onToggle={toggle}
              onCopy={(path, value) => void copyNode(path, value)}
              copiedPath={copiedPath}
              query=""
            />
          </div>
        )
      ) : (
        <pre className="scroll-area max-h-[60vh] overflow-auto whitespace-pre-wrap break-words rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 font-mono text-[12.5px] leading-relaxed text-zinc-300">
          {formatted}
        </pre>
      )}
    </div>
  );
}
