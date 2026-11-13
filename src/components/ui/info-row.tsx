export function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 text-[13px]">
      <span className="text-zinc-500">{label}</span>
      <span className="font-mono text-[12px] text-zinc-300">{value}</span>
    </div>
  );
}

export function InfoTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
      <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">{label}</p>
      <p className="mt-0.5 truncate font-mono text-[13px] text-zinc-200">{value}</p>
    </div>
  );
}
