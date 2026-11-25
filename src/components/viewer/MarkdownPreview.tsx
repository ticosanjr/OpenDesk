import { useMemo, useRef, useState, type ReactNode } from "react";
import { Check, Copy, ListTree, Search } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { CodeTextView } from "@/components/viewer/CodeTextView";

interface Heading {
  level: number;
  text: string;
  index: number;
}

function textOf(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (typeof node === "object" && "props" in node) {
    return textOf((node as { props: { children?: ReactNode } }).props.children);
  }
  return "";
}

function extractHeadings(content: string): Heading[] {
  const headings: Heading[] = [];
  let fenced = false;
  let index = 0;
  for (const line of content.split(/\r?\n/)) {
    if (/^\s*(```|~~~)/.test(line)) {
      fenced = !fenced;
      continue;
    }
    if (fenced) continue;
    const match = /^(#{1,3})\s+(.+?)\s*#*\s*$/.exec(line);
    if (!match) continue;
    headings.push({ level: match[1].length, text: match[2], index });
    index += 1;
  }
  return headings;
}

function CodeBlock({
  children,
  ...props
}: {
  children?: ReactNode;
  [key: string]: unknown;
}) {
  const [copied, setCopied] = useState(false);
  const code = textOf(children);
  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  }
  return (
    <div className="group relative">
      <Button
        variant="ghost"
        size="xs"
        onClick={() => void copy()}
        title="Copiar bloco de código"
        aria-label="Copiar bloco de código"
        className="absolute right-1 top-1 h-6 gap-1 bg-zinc-950/80 px-1.5 text-[10.5px] opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
      >
        {copied ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />}
        {copied ? "copiado" : "copiar"}
      </Button>
      <pre {...props}>{children}</pre>
    </div>
  );
}

export function MarkdownPreview({ content }: { content: string }) {
  const headings = useMemo(() => extractHeadings(content), [content]);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [query, setQuery] = useState("");
  const [showToc, setShowToc] = useState(false);
  const [copied, setCopied] = useState(false);

  const components = useMemo(
    () => ({
      pre: ({ node: _node, ...props }: { node?: unknown; children?: ReactNode }) => (
        <CodeBlock {...props} />
      ),
    }),
    [],
  );

  function jumpTo(heading: Heading) {
    const nodes = containerRef.current?.querySelectorAll("h1, h2, h3");
    nodes?.[heading.index]?.scrollIntoView?.({
      block: "start",
      behavior: "smooth",
    });
  }

  async function copyMarkdown() {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  }

  return (
    <div className="flex min-h-0 gap-3">
      {headings.length >= 3 ? (
        <nav
          aria-label="Sumário do documento"
          className="hidden w-56 shrink-0 overflow-auto rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-2 xl:block"
        >
          <p className="mb-1 px-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-zinc-500">
            Sumário
          </p>
          {headings.map((heading) => (
            <button
              key={`${heading.index}-${heading.text}`}
              type="button"
              onClick={() => jumpTo(heading)}
              className={cn(
                "block w-full truncate rounded px-1.5 py-0.5 text-left text-[12px] text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100",
                heading.level === 1 && "font-medium text-zinc-300",
                heading.level === 2 && "pl-3",
                heading.level === 3 && "pl-5 text-[11.5px]",
              )}
              title={heading.text}
            >
              {heading.text}
            </button>
          ))}
        </nav>
      ) : null}

      <div className="min-w-0 flex-1 space-y-2">
        <div className="flex flex-wrap items-center gap-1.5">
          {headings.length >= 3 ? (
            <Button
              variant={showToc ? "secondary" : "ghost"}
              size="sm"
              className="h-7 px-2 text-[11.5px] xl:hidden"
              onClick={() => setShowToc((value) => !value)}
            >
              <ListTree size={12} />
              Sumário
            </Button>
          ) : null}
          <label className="relative min-w-[160px] flex-1">
            <Search
              size={12}
              className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-zinc-500"
            />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Buscar no markdown"
              aria-label="Buscar no markdown"
              className="h-7 w-full rounded-md border border-zinc-800 bg-zinc-900/40 pl-7 pr-2 text-[12px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
            />
          </label>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-[11.5px]"
            onClick={() => void copyMarkdown()}
            title="Copiar markdown bruto"
          >
            {copied ? <Check size={12} className="text-emerald-400" /> : <Copy size={12} />}
            {copied ? "copiado" : "copiar markdown"}
          </Button>
        </div>

        {showToc && headings.length >= 3 ? (
          <div className="space-y-0.5 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-2 xl:hidden">
            {headings.map((heading) => (
              <button
                key={`${heading.index}-${heading.text}`}
                type="button"
                onClick={() => {
                  jumpTo(heading);
                  setShowToc(false);
                }}
                className="block w-full truncate rounded px-1.5 py-0.5 text-left text-[12px] text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
                style={{ paddingLeft: `${(heading.level - 1) * 10 + 6}px` }}
              >
                {heading.text}
              </button>
            ))}
          </div>
        ) : null}

        {query.trim() !== "" ? (
          <div className="space-y-1">
            <p className="text-[11.5px] text-zinc-500">
              Resultados no documento. Limpe a busca para voltar à visualização.
            </p>
            <CodeTextView
              key={query}
              content={content}
              name="markdown"
              initialQuery={query.trim()}
            />
          </div>
        ) : (
          <div ref={containerRef} className="markdown-body max-w-none">
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
              {content}
            </ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  );
}
