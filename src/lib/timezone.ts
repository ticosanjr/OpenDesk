export interface TimezoneOption {
  value: string;
  label: string;
  offsetMinutes: number;
}

const FALLBACK_ZONES = [
  "UTC",
  "America/Sao_Paulo",
  "America/New_York",
  "America/Los_Angeles",
  "Europe/London",
  "Europe/Lisbon",
  "Europe/Madrid",
  "Europe/Rome",
  "Europe/Berlin",
  "Africa/Johannesburg",
  "Asia/Dubai",
  "Asia/Kolkata",
  "Asia/Shanghai",
  "Asia/Tokyo",
  "Australia/Sydney",
];

export function formatOffset(minutes: number): string {
  const sign = minutes < 0 ? "-" : "+";
  const absolute = Math.abs(minutes);
  const hours = String(Math.floor(absolute / 60)).padStart(2, "0");
  const mins = String(absolute % 60).padStart(2, "0");
  return `UTC${sign}${hours}:${mins}`;
}

export function timezoneOffsetMinutes(zone: string, date = new Date()): number {
  try {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone: zone,
      timeZoneName: "longOffset",
    }).formatToParts(date);
    const label = parts.find((part) => part.type === "timeZoneName")?.value ?? "GMT";
    const match = label.match(/GMT([+-])(\d{2}):?(\d{2})?/);
    if (!match) return 0;
    const sign = match[1] === "-" ? -1 : 1;
    return sign * (Number(match[2]) * 60 + Number(match[3] ?? 0));
  } catch {
    return 0;
  }
}

function supportedTimezones(): string[] {
  const intl = Intl as unknown as {
    supportedValuesOf?: (key: string) => string[];
  };
  try {
    const values = intl.supportedValuesOf?.("timeZone") ?? [];
    return values.length ? values : FALLBACK_ZONES;
  } catch {
    return FALLBACK_ZONES;
  }
}

export function timezoneOptions(current?: string | null): TimezoneOption[] {
  const zones = new Set([...supportedTimezones(), ...FALLBACK_ZONES, "UTC"]);
  if (current?.trim()) zones.add(current.trim());
  return Array.from(zones)
    .map((zone) => {
      const offsetMinutes = timezoneOffsetMinutes(zone);
      return {
        value: zone,
        offsetMinutes,
        label: `(${formatOffset(offsetMinutes)}) ${zone}${
          current && zone === current ? " — seu fuso" : ""
        }`,
      };
    })
    .sort(
      (left, right) =>
        left.offsetMinutes - right.offsetMinutes ||
        left.value.localeCompare(right.value),
    );
}
