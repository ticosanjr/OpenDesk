import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  ArrowDown,
  ArrowUp,
  Check,
  Copy,
  Search,
  WrapText,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const PAGE_LINES = 2000;

function countMatches(text: string, query: string): number {
  if (!query) return 0;
  const haystack = text.toLowerCase();
  const needle = query.toLowerCase();
  let index = 0;
  let total = 0;
  while (index <= haystack.length - needle.length) {
    const found = haystack.indexOf(needle, index);
    if (found < 0) break;
    total += 1;
    index = found + Math.max(1, needle.length);
  }
  return total;
}

export function CodeTextView({
  content,
  name,
  initialQuery = "",
}: {
  content: string;
  name: string;
  initialQuery?: string;
}) {
  const lines = useMemo(() => content.split(/\r?\n/), [content]);
  const [query, setQuery] = useState(initialQuery);
  const [current, setCurrent] = useState(1);
  const [wrap, setWrap] = useState(true);
  const [copied, setCopied] = useState(false);
  const [visible, setVisible] = useState(PAGE_LINES);
  const lineRefs = useRef<Record<number, HTMLDivElement | null>>({});
  const activeQuery = query.trim();
  const total = useMemo(
    () => countMatches(content, activeQuery),
    [activeQuery, content],
  );

  useEffect(() => {
    setCurrent(1);
  }, [activeQuery]);

  const activeIndex = total === 0 ? 0 : ((current - 1) % total) + 1;
  const activeLine = useMemo(() => {
    if (!activeQuery || total === 0) return -1;
    const needle = activeQuery.toLowerCase();
    let seen = 0;
    for (let index = 0; index < lines.length; index += 1) {
      const line = lines[index].toLowerCase();
      let from = 0;
      while (from <= line.length - needle.length) {
        const found = line.indexOf(needle, from);
        if (found < 0) break;
        seen += 1;
        if (seen === activeIndex) return index;
        from = found + Math.max(1, needle.length);
      }
    }
    return -1;
  }, [activeIndex, activeQuery, lines, total]);

  useEffect(() => {
    if (activeLine >= 0) {
      lineRefs.current[activeLine]?.scrollIntoView?.({ block: "center" });
    }
  }, [activeLine]);

  function step(direction: 1 | -1) {
    if (total === 0) return;
    setCurrent((value) => ((value - 1 + direction + total) % total) + 1);
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  }

  let counter = 0;
  const rendered = lines.slice(0, visible).map((line, index) => {
    const nodes: ReactNode[] = [];
    if (activeQuery) {
      const haystack = line.toLowerCase();
      const needle = activeQuery.toLowerCase();
      let cursor = 0;
      let found = haystack.indexOf(needle, cursor);
      while (found >= 0) {
        if (found > cursor) nodes.push(line.slice(cursor, found));
        counter += 1;
        const occurrence = counter;
        nodes.push(
          <mark
            key={`${index}-${found}`}
            className={cn(
              "rounded px-0.5",
              occurrence === activeIndex
                ? "bg-amber-400/70 text-zinc-950"
                : "bg-amber-400/25 text-amber-100",
            )}
          >
            {line.slice(found, found + needle.length)}
          </mark>,
        );
        cursor = found + Math.max(1, needle.length);
        found = haystack.indexOf(needle, cursor);
      }
      if (cursor < line.length) nodes.push(line.slice(cursor));
    }
    return (
      <div
        key={index}
        ref={(node) => {
          lineRefs.current[index] = node;
        }}
        className={cn(
          "flex min-w-full",
          index === activeLine && "bg-blue-500/10",
        )}
      >
        <span className="sticky left-0 w-12 shrink-0 select-none border-r border-zinc-800/60 bg-zinc-950/95 pr-2 text-right font-mono text-[11.5px] leading-relaxed text-zinc-600">
          {index + 1}
        </span>
        <span
          className={cn(
            "flex-1 pl-2 font-mono text-[12.5px] leading-relaxed text-zinc-300",
            wrap ? "whitespace-pre-wrap break-words" : "whitespace-pre",
          )}
        >
          {nodes.length > 0 ? nodes : line || " "}
        </span>
      </div>
    );
  });

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex flex-wrap items-center gap-1.5">
        <label className="relative min-w-[170px] flex-1">
          <Search
            size={12}
            className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-zinc-500"
          />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                step(event.shiftKey ? -1 : 1);
              }
            }}
            placeholder={`Buscar em ${name}`}
            aria-label="Buscar no conteúdo"
            className="h-7 w-full rounded-md border border-zinc-800 bg-zinc-900/40 pl-7 pr-2 font-mono text-[12px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
          />
        </label>
        <span className="w-16 text-center font-mono text-[11.5px] text-zinc-500">
          {activeQuery ? `${total === 0 ? 0 : activeIndex}/${total}` : "—"}
        </span>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() => step(-1)}
          disabled={total === 0}
          aria-label="Ocorrência anterior"
          title="Ocorrência anterior"
        >
          <ArrowUp size={12} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() => step(1)}
          disabled={total === 0}
          aria-label="Próxima ocorrência"
          title="Próxima ocorrência"
        >
          <ArrowDown size={12} />
        </Button>
        <Button
          variant={wrap ? "secondary" : "ghost"}
          size="sm"
          className="h-7 px-2 text-[11.5px]"
          onClick={() => setWrap((value) => !value)}
          title="Alternar quebra de linha"
        >
          <WrapText size={12} />
          Wrap
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 px-2 text-[11.5px]"
          onClick={() => void copy()}
          title="Copiar o conteúdo"
        >
          {copied ? <Check size={12} className="text-emerald-400" /> : <Copy size={12} />}
          {copied ? "copiado" : "copiar"}
        </Button>
      </div>

      <div className="scroll-area max-h-[60vh] min-h-0 overflow-auto rounded-lg border border-zinc-800/70 bg-zinc-950/40 py-1">
        {rendered}
        {lines.length > visible ? (
          <div className="border-t border-zinc-800/70 px-3 py-2 text-center">
            <button
              type="button"
              onClick={() => setVisible((value) => value + PAGE_LINES)}
              className="text-[12px] text-blue-300 hover:text-blue-200"
            >
              Carregar mais {Math.min(PAGE_LINES, lines.length - visible)} linhas
            </button>
          </div>
        ) : null}
      </div>
    </div>
  );
}
