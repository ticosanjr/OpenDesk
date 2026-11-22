import { StudioField } from "@/components/studio/StudioList";
import { cn } from "@/lib/utils";
import { AVATAR_PRESETS, inputClass } from "./agent-presets";

export function AgentAvatarField({
  avatar,
  onChange,
}: {
  avatar: string | null | undefined;
  onChange: (avatar: string) => void;
}) {
  return (
    <StudioField label="Avatar" hint="Escolha um preset ou digite outro emoji.">
      <div className="space-y-2">
        <div className="grid grid-cols-10 gap-1">
          {AVATAR_PRESETS.map((preset) => (
            <button
              key={preset}
              type="button"
              title={preset}
              onClick={() => onChange(preset)}
              className={cn(
                "flex h-7 items-center justify-center rounded-md border text-[14px] transition",
                avatar === preset
                  ? "workspace-selected"
                  : "border-zinc-800 bg-zinc-900/40 hover:border-zinc-600",
              )}
            >
              {preset}
            </button>
          ))}
        </div>
        <div className="flex gap-2">
          <input
            value={avatar ?? ""}
            onChange={(event) =>
              onChange(event.target.value)
            }
            placeholder="🛡️"
            maxLength={8}
            className={cn(inputClass, "flex-1")}
          />
          <span className="flex h-9 w-9 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50 text-lg">
            {avatar || "🤖"}
          </span>
        </div>
      </div>
    </StudioField>
  );
}
