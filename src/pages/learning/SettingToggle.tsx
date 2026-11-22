import { Switch } from "@/components/ui/switch";

export function SettingToggle({
  label,
  description,
  checked,
  disabled = false,
  onCheckedChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-2.5 py-2">
      <div className="min-w-0">
        <p className="text-[12px] font-medium text-zinc-300">{label}</p>
        <p className="mt-0.5 text-[11.5px] leading-snug text-zinc-500">
          {description}
        </p>
      </div>
      <Switch
        checked={checked}
        onCheckedChange={onCheckedChange}
        disabled={disabled}
        label={label}
      />
    </div>
  );
}
