import { useState } from "react";
import { ChatPromptPreviewCard } from "./ChatPromptPreviewCard";
import { PromptPreviewCard } from "./PromptPreviewCard";
import { PromptTemplatesCard } from "./PromptTemplatesCard";
import { cn } from "@/lib/utils";

type Section = "templates" | "tasks" | "chat";

const SECTIONS: Array<{ id: Section; label: string }> = [
  { id: "templates", label: "Templates de prompts" },
  { id: "tasks", label: "Prompts enviados" },
  { id: "chat", label: "Prompts de chat" },
];

export function PromptSettings() {
  const [section, setSection] = useState<Section>("templates");
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap gap-1 rounded-xl border border-zinc-800/70 bg-zinc-950/40 p-1">
        {SECTIONS.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setSection(item.id)}
            className={cn(
              "rounded-lg px-3 py-1.5 text-[12.5px] transition-colors",
              section === item.id
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-200",
            )}
          >
            {item.label}
          </button>
        ))}
      </div>
      {section === "templates" ? <PromptTemplatesCard /> : null}
      {section === "tasks" ? <PromptPreviewCard /> : null}
      {section === "chat" ? <ChatPromptPreviewCard /> : null}
    </div>
  );
}
