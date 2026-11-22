import { Download, Loader2, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { OpenCodeUpdateCheck } from "@/lib/types";

export function EngineUpdateBox({
  check,
  daemonVersion,
  method,
  onMethodChange,
  busy,
  onCheckUpdate,
  onUpdate,
  updateOutput,
}: {
  check: OpenCodeUpdateCheck | null;
  daemonVersion: string | null | undefined;
  method: string;
  onMethodChange: (value: string) => void;
  busy: string | null;
  onCheckUpdate: () => void;
  onUpdate: () => void;
  updateOutput: string | null;
}) {
  return (
    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
      <div className="flex flex-wrap items-center gap-2">
        <p className="flex-1 text-[13px] text-zinc-300">
          OpenCode instalado:{" "}
          <span className="font-mono text-zinc-100">
            {check?.current ?? daemonVersion ?? "desconhecido"}
          </span>
          {check?.latest ? (
            <>
              {" "}
              · última V2:{" "}
              <span className="font-mono text-zinc-100">
                {check.latest}
              </span>
            </>
          ) : null}
        </p>
        <Button
          variant="outline"
          size="sm"
          onClick={() => onCheckUpdate()}
          disabled={busy !== null}
        >
          {busy === "check" ? (
            <Loader2 size={13} className="animate-spin" />
          ) : (
            <Search size={13} />
          )}
          Verificar atualização
        </Button>
        {check?.update_available ? (
          <>
            <label className="flex items-center gap-1 text-[11.5px] text-zinc-500">
              método
              <select
                value={method}
                onChange={(event) => onMethodChange(event.target.value)}
                className="h-7 rounded-md border border-zinc-800 bg-zinc-900/60 px-2 text-[12px] text-zinc-200 outline-none focus:border-blue-500/60"
              >
                {["curl", "npm", "pnpm", "bun", "yarn", "brew", "winget"].map(
                  (value) => (
                    <option key={value} value={value}>
                      {value}
                    </option>
                  ),
                )}
              </select>
            </label>
            <Button
              size="sm"
              onClick={() => onUpdate()}
              disabled={busy !== null}
            >
              {busy === "update" ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Download size={13} />
              )}
              Atualizar OpenCode
            </Button>
          </>
        ) : null}
      </div>
      {check?.release?.notes ? (
        <p className="mt-2 max-h-24 overflow-y-auto whitespace-pre-wrap text-[11.5px] leading-relaxed text-zinc-500">
          {check.release.notes}
        </p>
      ) : null}
      {updateOutput ? (
        <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap rounded bg-black/40 p-2 font-mono text-[11.5px] text-zinc-400">
          {updateOutput}
        </pre>
      ) : null}
    </div>
  );
}
