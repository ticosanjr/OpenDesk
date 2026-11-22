import type { Dispatch, SetStateAction } from "react";
import { Loader2, ShieldCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { inputClass, type SandboxStatus } from "./types";

export function SandboxPolicyCard({
  sandbox,
  setSandbox,
  commands,
  paths,
  onCommandsChange,
  onPathsChange,
  busy,
  onSave,
}: {
  sandbox: SandboxStatus | null;
  setSandbox: Dispatch<SetStateAction<SandboxStatus | null>>;
  commands: string;
  paths: string;
  onCommandsChange: (value: string) => void;
  onPathsChange: (value: string) => void;
  busy: string | null;
  onSave: () => void;
}) {
  return (
    <Card id="settings-sandbox" className="scroll-mt-14">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <ShieldCheck size={15} className="text-zinc-500" />
            Sandbox de execução
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Vale para harnesses CLI. `workspace` limpa credenciais e trava o cwd;
            `strict` usa bubblewrap (sem fallback silencioso).
          </p>
        </div>
        {sandbox ? (
          <div className="flex items-center gap-1.5">
            <Badge variant={sandbox.scope === "workspace" ? "blue" : "default"}>
              {sandbox.scope}
            </Badge>
            <Badge variant={sandbox.bwrap_available ? "green" : "default"}>
              bwrap {sandbox.bwrap_available ? "ok" : "ausente"}
            </Badge>
            <Badge variant={sandbox.container_runtime ? "green" : "default"}>
              {sandbox.container_runtime ?? "container ausente"}
            </Badge>
          </div>
        ) : null}
      </CardHeader>
      <CardContent className="space-y-2">
        {sandbox ? (
          <>
            <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
              <select
                value={sandbox.policy.mode}
                onChange={(event) =>
                  setSandbox({
                    ...sandbox,
                    policy: { ...sandbox.policy, mode: event.target.value },
                  })
                }
                className={inputClass}
              >
                <option value="off">off</option>
                <option value="workspace">workspace</option>
                <option value="strict">strict (bwrap)</option>
                <option value="container">container (docker/podman)</option>
              </select>
              <input
                value={sandbox.policy.max_runtime_minutes}
                onChange={(event) =>
                  setSandbox({
                    ...sandbox,
                    policy: {
                      ...sandbox.policy,
                      max_runtime_minutes: Number(event.target.value) || 0,
                    },
                  })
                }
                inputMode="numeric"
                placeholder="minutos (0 = padrão)"
                className={inputClass}
              />
              <label className="flex items-center gap-2 text-[12.5px] text-zinc-300">
                <input
                  type="checkbox"
                  checked={sandbox.policy.network}
                  onChange={(event) =>
                    setSandbox({
                      ...sandbox,
                      policy: { ...sandbox.policy, network: event.target.checked },
                    })
                  }
                  className="h-4 w-4 accent-blue-500"
                />
                rede no sandbox
              </label>
              <label className="flex items-center gap-2 text-[12.5px] text-zinc-300">
                <input
                  type="checkbox"
                  checked={sandbox.policy.scrub_env}
                  onChange={(event) =>
                    setSandbox({
                      ...sandbox,
                      policy: { ...sandbox.policy, scrub_env: event.target.checked },
                    })
                  }
                  className="h-4 w-4 accent-blue-500"
                />
                limpar credenciais do env
              </label>
            </div>
            {sandbox.policy.mode === "container" ? (
              <input
                value={sandbox.policy.image}
                onChange={(event) =>
                  setSandbox({
                    ...sandbox,
                    policy: { ...sandbox.policy, image: event.target.value },
                  })
                }
                placeholder="imagem única (ex.: opendesk/sandbox:latest)"
                className={inputClass}
              />
            ) : null}
            <input
              value={commands}
              onChange={(event) => onCommandsChange(event.target.value)}
              placeholder="allowlist de comandos (vírgula; vazio = todos)"
              className={inputClass}
            />
            <input
              value={paths}
              onChange={(event) => onPathsChange(event.target.value)}
              placeholder="paths extras read-write no strict/container (vírgula)"
              className={inputClass}
            />
            <div className="flex justify-end">
              <Button
                size="sm"
                className="h-7"
                disabled={busy === "sandbox"}
                onClick={() => onSave()}
              >
                Salvar política
              </Button>
            </div>
          </>
        ) : (
          <Loader2 size={14} className="animate-spin text-zinc-500" />
        )}
      </CardContent>
    </Card>
  );
}
