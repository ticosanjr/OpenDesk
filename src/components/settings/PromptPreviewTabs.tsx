import { cn } from "@/lib/utils";

export type PromptPreviewView = "sections" | "desk" | "opencode" | "final";

const TABS: Array<{ id: PromptPreviewView; label: string }> = [
  { id: "sections", label: "Seções" },
  { id: "desk", label: "OpenDesk" },
  { id: "opencode", label: "OpenCode" },
  { id: "final", label: "Final reconstruído" },
];

export function PromptPreviewTabs({
  view,
  onChange,
}: {
  view: PromptPreviewView;
  onChange: (view: PromptPreviewView) => void;
}) {
  return (
    <div className="flex rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
      {TABS.map((tab) => (
        <button
          key={tab.id}
          type="button"
          onClick={() => onChange(tab.id)}
          className={cn(
            "rounded-md px-2 py-1 text-[12px]",
            view === tab.id
              ? "bg-zinc-800 text-zinc-100"
              : "text-zinc-500 hover:text-zinc-300",
          )}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
