import { useMemo, useState } from "react";
import { ArrowDown, ArrowUp, ArrowUpDown, Search } from "lucide-react";
import { cn } from "@/lib/utils";

const PAGE_ROWS = 500;

export function parseDelimited(text: string, delimiter: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (quoted) {
      if (char === '"') {
        if (text[index + 1] === '"') {
          field += '"';
          index += 1;
        } else {
          quoted = false;
        }
      } else {
        field += char;
      }
      continue;
    }
    if (char === '"' && field === "") {
      quoted = true;
    } else if (char === delimiter) {
      row.push(field);
      field = "";
    } else if (char === "\n") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
    } else if (char !== "\r") {
      field += char;
    }
  }
  if (field !== "" || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows.filter((cells) => cells.some((cell) => cell.trim() !== ""));
}

function delimiterFor(name: string, text: string): string {
  if (/\.tsv$/i.test(name)) return "\t";
  const firstLine = text.split(/\r?\n/, 1)[0] ?? "";
  const tabs = (firstLine.match(/\t/g) ?? []).length;
  const commas = (firstLine.match(/,/g) ?? []).length;
  return tabs > commas ? "\t" : ",";
}

function compare(a: string, b: string): number {
  const left = Number(a);
  const right = Number(b);
  if (a.trim() !== "" && b.trim() !== "" && !Number.isNaN(left) && !Number.isNaN(right)) {
    return left - right;
  }
  return a.localeCompare(b, "pt-BR", { numeric: true, sensitivity: "base" });
}

export function CsvTableView({ name, content }: { name: string; content: string }) {
  const delimiter = useMemo(() => delimiterFor(name, content), [content, name]);
  const rows = useMemo(() => parseDelimited(content, delimiter), [content, delimiter]);
  const [view, setView] = useState<"table" | "raw">("table");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ column: number; direction: "asc" | "desc" } | null>(
    null,
  );
  const [visibleRows, setVisibleRows] = useState(PAGE_ROWS);

  const header = rows[0] ?? [];
  const body = rows.slice(1);
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const base = needle
      ? body.filter((cells) =>
          cells.some((cell) => cell.toLowerCase().includes(needle)),
        )
      : body;
    if (!sort) return base;
    const sorted = [...base].sort((left, right) =>
      compare(left[sort.column] ?? "", right[sort.column] ?? ""),
    );
    return sort.direction === "asc" ? sorted : sorted.reverse();
  }, [body, query, sort]);

  function toggleSort(column: number) {
    setSort((current) => {
      if (current?.column !== column) return { column, direction: "asc" };
      return current.direction === "asc"
        ? { column, direction: "desc" }
        : null;
    });
  }

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="flex items-center gap-1 rounded-md border border-zinc-800 p-0.5">
          {(["table", "raw"] as const).map((value) => (
            <button
              key={value}
              type="button"
              onClick={() => setView(value)}
              className={cn(
                "rounded px-1.5 py-0.5 text-[12px]",
                view === value
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:text-zinc-200",
              )}
            >
              {value === "table" ? "Tabela" : "Bruto"}
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
            onChange={(event) => {
              setQuery(event.target.value);
              setVisibleRows(PAGE_ROWS);
            }}
            placeholder="Filtrar linhas"
            aria-label="Filtrar linhas da tabela"
            className="h-7 w-full rounded-md border border-zinc-800 bg-zinc-900/40 pl-7 pr-2 text-[12px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
          />
        </label>
        <span className="font-mono text-[11.5px] text-zinc-500">
          {filtered.length} de {body.length} linhas · {header.length} colunas
        </span>
      </div>

      {view === "raw" ? (
        <pre className="scroll-area max-h-[60vh] overflow-auto whitespace-pre-wrap break-words rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 font-mono text-[12.5px] leading-relaxed text-zinc-300">
          {content}
        </pre>
      ) : (
        <div className="scroll-area max-h-[60vh] overflow-auto rounded-lg border border-zinc-800/70">
          <table className="w-full border-collapse text-left text-[12.5px]">
            <thead className="sticky top-0 z-10 bg-zinc-950/95">
              <tr>
                {header.map((cell, column) => (
                  <th
                    key={`${column}-${cell}`}
                    className="whitespace-nowrap border-b border-zinc-800 px-2 py-1.5 font-medium text-zinc-300"
                  >
                    <button
                      type="button"
                      onClick={() => toggleSort(column)}
                      title={`Ordenar por ${cell || `coluna ${column + 1}`}`}
                      className="inline-flex items-center gap-1 hover:text-white"
                    >
                      {cell || `coluna ${column + 1}`}
                      {sort?.column === column ? (
                        sort.direction === "asc" ? (
                          <ArrowUp size={11} className="text-blue-300" />
                        ) : (
                          <ArrowDown size={11} className="text-blue-300" />
                        )
                      ) : (
                        <ArrowUpDown size={11} className="text-zinc-600" />
                      )}
                    </button>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {filtered.slice(0, visibleRows).map((cells, rowIndex) => (
                <tr key={rowIndex} className="odd:bg-zinc-900/30">
                  {header.map((_, column) => (
                    <td
                      key={column}
                      className="max-w-[360px] truncate border-b border-zinc-800/50 px-2 py-1 font-mono text-zinc-300"
                      title={cells[column] ?? ""}
                    >
                      {cells[column] ?? ""}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
          {filtered.length === 0 ? (
            <p className="px-3 py-6 text-center text-[12.5px] text-zinc-500">
              Nenhuma linha corresponde ao filtro.
            </p>
          ) : null}
          {filtered.length > visibleRows ? (
            <div className="border-t border-zinc-800/70 px-3 py-2 text-center">
              <button
                type="button"
                onClick={() => setVisibleRows((current) => current + PAGE_ROWS)}
                className="text-[12px] text-blue-300 hover:text-blue-200"
              >
                Carregar mais {Math.min(PAGE_ROWS, filtered.length - visibleRows)} linhas
              </button>
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
}
