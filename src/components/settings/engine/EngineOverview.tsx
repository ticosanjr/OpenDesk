import { InfoTile } from "@/components/ui/info-row";
import { shortPath } from "@/lib/format";
import type { DaemonStatus, EngineStatus } from "@/lib/types";

export function EngineOverview({
  engine,
  daemon,
  online,
}: {
  engine: EngineStatus | null;
  daemon: DaemonStatus | undefined;
  online: boolean;
}) {
  return (
    <>
      <div className="grid gap-2 md:grid-cols-4">
        <InfoTile label="Versão" value={daemon?.version ?? "—"} />
        <InfoTile label="Porta" value={daemon?.port ? String(daemon.port) : "—"} />
        <InfoTile
          label="Binário"
          value={
            daemon?.binary_path
              ? shortPath(daemon.binary_path, 48)
              : engine?.resolved?.path
                ? shortPath(engine.resolved.path, 48)
                : "não detectado"
          }
        />
        <InfoTile
          label="Método"
          value={engine?.resolved?.method ?? "desconhecido"}
        />
      </div>

      {!online && (daemon?.message || engine?.resolve_error) ? (
        <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] leading-relaxed text-amber-200">
          {daemon?.message ?? engine?.resolve_error}
        </p>
      ) : null}
    </>
  );
}
