import { useCallback, useEffect, useState } from "react";
import {
  Camera,
  Loader2,
  Monitor,
  MousePointerClick,
  ShieldCheck,
  Type,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";

interface SandboxPolicy {
  mode: string;
  network: boolean;
  allowed_paths: string[];
  allowed_commands: string[];
  max_runtime_minutes: number;
  scrub_env: boolean;
  image: string;
}

interface SandboxStatus {
  policy: SandboxPolicy;
  scope: string;
  workspace_id?: string | null;
  bwrap_available: boolean;
  container_runtime?: string | null;
  platform: string;
}

interface Capabilities {
  available: boolean;
  backend: string;
  display?: string | null;
  screenshot_tool?: string | null;
  input_tool?: string | null;
  reason?: string | null;
}

interface ComputerSession {
  id: string;
  status: string;
  require_approval: boolean;
  actions: number;
  screenshot_path?: string | null;
  created_at: string;
}

const inputClass =
  "h-8 w-full rounded border border-zinc-700 bg-zinc-950 px-2 text-[12.5px] text-zinc-200";

export function SandboxComputerCard({ workspaceId }: { workspaceId: string }) {
  const [sandbox, setSandbox] = useState<SandboxStatus | null>(null);
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null);
  const [sessions, setSessions] = useState<ComputerSession[]>([]);
  const [commands, setCommands] = useState("");
  const [paths, setPaths] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [requireApproval, setRequireApproval] = useState(true);
  const [action, setAction] = useState({ kind: "type", text: "", key: "", x: "0", y: "0" });

  const load = useCallback(async () => {
    if (!workspaceId) return;
    try {
      const [sandboxStatus, capabilitiesStatus, sessionList] = await Promise.all([
        apiGet<SandboxStatus>(
          `/api/sandbox?workspace_id=${encodeURIComponent(workspaceId)}`,
        ),
        apiGet<Capabilities>("/api/computer/status"),
        apiGet<ComputerSession[]>(
          `/api/computer/sessions?workspace_id=${encodeURIComponent(workspaceId)}`,
        ),
      ]);
      setSandbox(sandboxStatus);
      setCapabilities(capabilitiesStatus);
      setSessions(sessionList);
      setCommands(sandboxStatus.policy.allowed_commands.join(", "));
      setPaths(sandboxStatus.policy.allowed_paths.join(", "));
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar o sandbox",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }, [workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function saveSandbox() {
    if (!sandbox) return;
    setBusy("sandbox");
    try {
      const policy = await apiPut<SandboxPolicy>(
        `/api/sandbox?workspace_id=${encodeURIComponent(workspaceId)}`,
        {
          ...sandbox.policy,
          allowed_commands: commands
            .split(",")
            .map((value) => value.trim())
            .filter(Boolean),
          allowed_paths: paths
            .split(",")
            .map((value) => value.trim())
            .filter(Boolean),
        },
      );
      setSandbox({ ...sandbox, policy, scope: "workspace" });
      toast({ tone: "success", title: "Política de sandbox salva" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao salvar o sandbox",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function createSession() {
    setBusy("create");
    try {
      await apiPost("/api/computer/sessions", {
        workspace_id: workspaceId,
        require_approval: requireApproval,
      });
      await load();
      toast({
        tone: "success",
        title: "Sessão de computer-use criada",
        description: requireApproval
          ? "Aprove a sessão antes de enviar ações."
          : "Sessão ativa.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Computer-use indisponível",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function sessionAction(id: string, kind: "approve" | "end") {
    setBusy(`${kind}:${id}`);
    try {
      await apiPost(`/api/computer/sessions/${id}/${kind}`);
      await load();
    } finally {
      setBusy(null);
    }
  }

  async function capture(id: string) {
    setBusy(`shot:${id}`);
    try {
      const result = await apiPost<{ path: string; data: string }>(
        `/api/computer/sessions/${id}/screenshot`,
      );
      setPreview(`data:image/png;base64,${result.data}`);
      toast({ tone: "success", title: "Captura salva", description: result.path });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha na captura",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function sendAction(session: ComputerSession) {
    setBusy(`act:${session.id}`);
    try {
      const payload =
        action.kind === "type"
          ? { action: "type", text: action.text }
          : action.kind === "key"
            ? { action: "key", key: action.key }
            : { action: "click", x: Number(action.x) || 0, y: Number(action.y) || 0 };
      await apiPost(`/api/computer/sessions/${session.id}/action`, payload);
      setAction({ ...action, text: "" });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Ação recusada",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  const active = sessions.find((session) => session.status === "ACTIVE");

  return (
    <div className="space-y-4">
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
                onChange={(event) => setCommands(event.target.value)}
                placeholder="allowlist de comandos (vírgula; vazio = todos)"
                className={inputClass}
              />
              <input
                value={paths}
                onChange={(event) => setPaths(event.target.value)}
                placeholder="paths extras read-write no strict/container (vírgula)"
                className={inputClass}
              />
              <div className="flex justify-end">
                <Button
                  size="sm"
                  className="h-7"
                  disabled={busy === "sandbox"}
                  onClick={() => void saveSandbox()}
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

      <Card id="settings-computer" className="scroll-mt-14">
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Monitor size={15} className="text-zinc-500" />
              Computer-use
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Capturas e entrada no desktop real via ferramentas do host. Ações
              são contadas e resumidas; texto digitado nunca é armazenado.
            </p>
          </div>
          {capabilities ? (
            <Badge variant={capabilities.available ? "green" : "red"}>
              {capabilities.available ? capabilities.backend : "indisponível"}
            </Badge>
          ) : null}
        </CardHeader>
        <CardContent className="space-y-3">
          {capabilities && !capabilities.available ? (
            <p className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-[12px] text-amber-200">
              {capabilities.reason}
            </p>
          ) : null}
          <div className="flex flex-wrap items-center gap-2">
            <label className="flex items-center gap-2 text-[12.5px] text-zinc-300">
              <input
                type="checkbox"
                checked={requireApproval}
                onChange={(event) => setRequireApproval(event.target.checked)}
                className="h-4 w-4 accent-blue-500"
              />
              exigir aprovação humana
            </label>
            <Button
              size="sm"
              className="h-7"
              disabled={busy === "create" || !capabilities?.available}
              onClick={() => void createSession()}
            >
              Nova sessão
            </Button>
            {capabilities ? (
              <span className="font-mono text-[11px] text-zinc-600">
                {capabilities.screenshot_tool ?? "?"} · {capabilities.input_tool ?? "?"}
              </span>
            ) : null}
          </div>

          {sessions.length === 0 ? (
            <EmptyState
              title="Nenhuma sessão"
              description="Crie uma sessão para capturar a tela ou enviar entrada."
            />
          ) : (
            <div className="space-y-1.5">
              {sessions.map((session) => (
                <div
                  key={session.id}
                  className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
                >
                  <div>
                    <div className="flex items-center gap-1.5">
                      <Badge
                        variant={
                          session.status === "ACTIVE"
                            ? "green"
                            : session.status === "PENDING"
                              ? "yellow"
                              : "default"
                        }
                      >
                        {session.status}
                      </Badge>
                      <span className="font-mono text-[11px] text-zinc-500">
                        {session.id.slice(0, 8)} · {session.actions} ação(ões)
                      </span>
                    </div>
                    {session.screenshot_path ? (
                      <p className="truncate font-mono text-[10.5px] text-zinc-600">
                        {session.screenshot_path}
                      </p>
                    ) : null}
                  </div>
                  <div className="flex items-center gap-1.5">
                    {session.status === "PENDING" ? (
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        disabled={busy === `approve:${session.id}`}
                        onClick={() => void sessionAction(session.id, "approve")}
                      >
                        Aprovar
                      </Button>
                    ) : null}
                    {session.status === "ACTIVE" ? (
                      <>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7"
                          disabled={busy === `shot:${session.id}`}
                          onClick={() => void capture(session.id)}
                        >
                          <Camera size={11} />
                          Capturar
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7"
                          disabled={busy === `end:${session.id}`}
                          onClick={() => void sessionAction(session.id, "end")}
                        >
                          Encerrar
                        </Button>
                      </>
                    ) : null}
                  </div>
                </div>
              ))}
            </div>
          )}

          {active ? (
            <div className="grid gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 sm:grid-cols-2 lg:grid-cols-5">
              <select
                value={action.kind}
                onChange={(event) => setAction({ ...action, kind: event.target.value })}
                className={inputClass}
              >
                <option value="type">digitar</option>
                <option value="key">tecla</option>
                <option value="click">clique</option>
              </select>
              {action.kind === "type" ? (
                <input
                  value={action.text}
                  onChange={(event) => setAction({ ...action, text: event.target.value })}
                  placeholder="texto"
                  className={`${inputClass} sm:col-span-2`}
                />
              ) : action.kind === "key" ? (
                <input
                  value={action.key}
                  onChange={(event) => setAction({ ...action, key: event.target.value })}
                  placeholder="ex.: ctrl+s"
                  className={inputClass}
                />
              ) : (
                <>
                  <input
                    value={action.x}
                    onChange={(event) => setAction({ ...action, x: event.target.value })}
                    placeholder="x"
                    className={inputClass}
                  />
                  <input
                    value={action.y}
                    onChange={(event) => setAction({ ...action, y: event.target.value })}
                    placeholder="y"
                    className={inputClass}
                  />
                </>
              )}
              <Button
                size="sm"
                className="h-8"
                disabled={busy === `act:${active.id}`}
                onClick={() => void sendAction(active)}
              >
                {action.kind === "type" ? (
                  <Type size={12} />
                ) : (
                  <MousePointerClick size={12} />
                )}
                Enviar
              </Button>
            </div>
          ) : null}

          {preview ? (
            <div className="overflow-hidden rounded-lg border border-zinc-800/70">
              <img src={preview} alt="Captura da sessão de computer-use" className="w-full" />
            </div>
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}
