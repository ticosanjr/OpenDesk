import { AlertTriangle } from "lucide-react";

export function EngineFeedback({
  message,
  error,
  candidates,
}: {
  message: string | null;
  error: string | null;
  candidates: string[] | undefined;
}) {
  return (
    <>
      {message ? (
        <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[12px] leading-relaxed text-emerald-300">
          {message}
        </p>
      ) : null}
      {error ? (
        <p className="flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] leading-relaxed text-red-300">
          <AlertTriangle size={12} className="mt-0.5 shrink-0" />
          {error}
        </p>
      ) : null}

      {candidates && candidates.length > 0 ? (
        <details className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[11.5px] text-zinc-500">
          <summary className="cursor-pointer text-zinc-400">
            Caminhos testados ({candidates.length})
          </summary>
          <ul className="mt-2 space-y-0.5 font-mono">
            {candidates.slice(0, 24).map((candidate) => (
              <li key={candidate} className="truncate">
                {candidate}
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </>
  );
}
