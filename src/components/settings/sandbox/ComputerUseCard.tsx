import { Camera, Monitor, MousePointerClick, Type } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import {
  inputClass,
  type Capabilities,
  type ComputerAction,
  type ComputerSession,
  type WindowAllowlist,
} from "./types";

export function ComputerUseCard({
  capabilities,
  sessions,
  allowlist,
  allowWindows,
  allowApps,
  onAllowlistChange,
  onAllowWindowsChange,
  onAllowAppsChange,
  requireApproval,
  onRequireApprovalChange,
  isolated,
  onIsolatedChange,
  busy,
  action,
  onActionChange,
  active,
  preview,
  onCreate,
  onSaveAllowlist,
  onSessionAction,
  onCapture,
  onSendAction,
}: {
  capabilities: Capabilities | null;
  sessions: ComputerSession[];
  allowlist: WindowAllowlist;
  allowWindows: string;
  allowApps: string;
  onAllowlistChange: (value: WindowAllowlist) => void;
  onAllowWindowsChange: (value: string) => void;
  onAllowAppsChange: (value: string) => void;
  requireApproval: boolean;
  onRequireApprovalChange: (value: boolean) => void;
  isolated: boolean;
  onIsolatedChange: (value: boolean) => void;
  busy: string | null;
  action: ComputerAction;
  onActionChange: (value: ComputerAction) => void;
  active: ComputerSession | undefined;
  preview: string | null;
  onCreate: () => void;
  onSaveAllowlist: () => void;
  onSessionAction: (id: string, kind: "approve" | "end") => void;
  onCapture: (id: string) => void;
  onSendAction: (session: ComputerSession) => void;
}) {
  return (
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
              onChange={(event) => onRequireApprovalChange(event.target.checked)}
              className="h-4 w-4 accent-blue-500"
            />
            exigir aprovação humana
          </label>
          <label className="flex items-center gap-2 text-[12.5px] text-zinc-300">
            <input
              type="checkbox"
              checked={isolated}
              onChange={(event) => onIsolatedChange(event.target.checked)}
              className="h-4 w-4 accent-blue-500"
            />
            display isolado (Xvfb)
          </label>
          <Button
            size="sm"
            className="h-7"
            disabled={busy === "create" || (!isolated && !capabilities?.available)}
            onClick={() => onCreate()}
          >
            Nova sessão
          </Button>
          {capabilities ? (
            <span className="font-mono text-[11px] text-zinc-600">
              {capabilities.screenshot_tool ?? "?"} · {capabilities.input_tool ?? "?"}
            </span>
          ) : null}
        </div>

        <div className="space-y-2 rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
          <label className="flex items-center gap-2 text-[12.5px] text-zinc-300">
            <input
              type="checkbox"
              checked={allowlist.enabled}
              onChange={(event) =>
                onAllowlistChange({ ...allowlist, enabled: event.target.checked })
              }
              className="h-4 w-4 accent-blue-500"
            />
            allowlist de janela/app para entrada e captura
          </label>
          <div className="grid gap-2 sm:grid-cols-2">
            <input
              value={allowWindows}
              onChange={(event) => onAllowWindowsChange(event.target.value)}
              placeholder="janelas (título contém; vírgula)"
              className={inputClass}
            />
            <input
              value={allowApps}
              onChange={(event) => onAllowAppsChange(event.target.value)}
              placeholder="apps (processo contém; vírgula)"
              className={inputClass}
            />
          </div>
          <div className="flex justify-end">
            <Button
              size="sm"
              variant="outline"
              className="h-7"
              disabled={busy === "allowlist"}
              onClick={() => onSaveAllowlist()}
            >
              Salvar allowlist
            </Button>
          </div>
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
                      {session.isolated && session.display
                        ? ` · isolada ${session.display}`
                        : ""}
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
                      onClick={() => onSessionAction(session.id, "approve")}
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
                        onClick={() => onCapture(session.id)}
                      >
                        <Camera size={11} />
                        Capturar
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        disabled={busy === `end:${session.id}`}
                        onClick={() => onSessionAction(session.id, "end")}
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
              onChange={(event) => onActionChange({ ...action, kind: event.target.value })}
              className={inputClass}
            >
              <option value="type">digitar</option>
              <option value="key">tecla</option>
              <option value="click">clique</option>
            </select>
            {action.kind === "type" ? (
              <input
                value={action.text}
                onChange={(event) => onActionChange({ ...action, text: event.target.value })}
                placeholder="texto"
                className={`${inputClass} sm:col-span-2`}
              />
            ) : action.kind === "key" ? (
              <input
                value={action.key}
                onChange={(event) => onActionChange({ ...action, key: event.target.value })}
                placeholder="ex.: ctrl+s"
                className={inputClass}
              />
            ) : (
              <>
                <input
                  value={action.x}
                  onChange={(event) => onActionChange({ ...action, x: event.target.value })}
                  placeholder="x"
                  className={inputClass}
                />
                <input
                  value={action.y}
                  onChange={(event) => onActionChange({ ...action, y: event.target.value })}
                  placeholder="y"
                  className={inputClass}
                />
              </>
            )}
            <Button
              size="sm"
              className="h-8"
              disabled={busy === `act:${active.id}`}
              onClick={() => onSendAction(active)}
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
  );
}
