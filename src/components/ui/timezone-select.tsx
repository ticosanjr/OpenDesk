import { useMemo } from "react";
import { cn } from "@/lib/utils";
import { timezoneOptions } from "@/lib/timezone";

export function TimezoneSelect({
  value,
  onChange,
  className,
  disabled = false,
  id,
}: {
  value: string;
  onChange: (value: string) => void;
  className?: string;
  disabled?: boolean;
  id?: string;
}) {
  const options = useMemo(() => timezoneOptions(value), [value]);
  return (
    <select
      id={id}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      className={cn(className)}
      aria-label="Fuso horário"
      disabled={disabled}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}
