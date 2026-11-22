import { useRef } from "react";

export function LocalYamlEditor({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const textarea = useRef<HTMLTextAreaElement | null>(null);
  const gutter = useRef<HTMLDivElement | null>(null);
  const lines = value.split("\n").length;

  return (
    <div className="flex h-full min-h-0 bg-zinc-950">
      <div
        ref={gutter}
        aria-hidden
        className="h-full select-none overflow-hidden border-r border-zinc-800/70 px-2 py-3 text-right font-mono text-[12px] leading-[1.6] text-zinc-500"
      >
        {Array.from({ length: lines }).map((_, index) => (
          <div key={index}>{index + 1}</div>
        ))}
      </div>
      <textarea
        ref={textarea}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onScroll={(event) => {
          if (gutter.current) {
            gutter.current.scrollTop = event.currentTarget.scrollTop;
          }
        }}
        spellCheck={false}
        className="h-full min-h-0 flex-1 resize-none bg-transparent p-3 font-mono text-[13px] leading-[1.6] text-zinc-200 outline-none"
        aria-label="Editor YAML da receita"
      />
    </div>
  );
}
