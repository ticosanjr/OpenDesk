import { StudioField } from "@/components/studio/StudioList";
import { cn } from "@/lib/utils";
import { COLOR_PRESETS } from "./agent-presets";

export function AgentColorField({
  color,
  onChange,
}: {
  color: string | null | undefined;
  onChange: (color: string) => void;
}) {
  return (
    <StudioField label="Cor" hint="Presets ou cor personalizada.">
      <div className="space-y-2">
        <div className="flex flex-wrap gap-1.5">
          {COLOR_PRESETS.map((preset) => (
            <button
              key={preset}
              type="button"
              title={preset}
              onClick={() => onChange(preset)}
              className={cn(
                "h-6 w-6 rounded-full border-2 transition",
                (color || "#60a5fa") === preset
                  ? "border-white/80"
                  : "border-transparent hover:border-zinc-500",
              )}
              style={{ background: preset }}
            />
          ))}
        </div>
        <div className="flex h-9 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2">
          <input
            type="color"
            value={color || "#60a5fa"}
            onChange={(event) =>
              onChange(event.target.value)
            }
            className="h-5 w-8 cursor-pointer rounded border-0 bg-transparent p-0"
          />
          <span className="font-mono text-[12px] text-zinc-500">
            {color || "#60a5fa"}
          </span>
        </div>
      </div>
    </StudioField>
  );
}
