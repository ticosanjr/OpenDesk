import { useCallback, useEffect, useState } from "react";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";
import { ComputerUseCard } from "./sandbox/ComputerUseCard";
import { SandboxPolicyCard } from "./sandbox/SandboxPolicyCard";
import type {
  Capabilities,
  ComputerAction,
  ComputerSession,
  SandboxPolicy,
  SandboxStatus,
  WindowAllowlist,
} from "./sandbox/types";

export function SandboxComputerCard({ workspaceId }: { workspaceId: string }) {
  const [sandbox, setSandbox] = useState<SandboxStatus | null>(null);
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null);
  const [sessions, setSessions] = useState<ComputerSession[]>([]);
  const [commands, setCommands] = useState("");
  const [paths, setPaths] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [requireApproval, setRequireApproval] = useState(true);
  const [isolated, setIsolated] = useState(true);
  const [allowlist, setAllowlist] = useState<WindowAllowlist>({
    enabled: false,
    windows: [],
    apps: [],
  });
  const [allowWindows, setAllowWindows] = useState("");
  const [allowApps, setAllowApps] = useState("");
  const [action, setAction] = useState<ComputerAction>({
    kind: "type",
    text: "",
    key: "",
    x: "0",
    y: "0",
  });

  const load = useCallback(async () => {
    if (!workspaceId) return;
    try {
      const [sandboxStatus, capabilitiesStatus, sessionList, allowlistStatus] =
        await Promise.all([
          apiGet<SandboxStatus>(
            `/api/sandbox?workspace_id=${encodeURIComponent(workspaceId)}`,
          ),
          apiGet<Capabilities>("/api/computer/status"),
          apiGet<ComputerSession[]>(
            `/api/computer/sessions?workspace_id=${encodeURIComponent(workspaceId)}`,
          ),
          apiGet<WindowAllowlist>("/api/computer/allowlist"),
        ]);
      setSandbox(sandboxStatus);
      setCapabilities(capabilitiesStatus);
      setSessions(sessionList);
      setCommands(sandboxStatus.policy.allowed_commands.join(", "));
      setPaths(sandboxStatus.policy.allowed_paths.join(", "));
      setAllowlist(allowlistStatus);
      setAllowWindows(allowlistStatus.windows.join(", "));
      setAllowApps(allowlistStatus.apps.join(", "));
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
        isolated,
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

  async function saveAllowlist() {
    setBusy("allowlist");
    try {
      const saved = await apiPut<WindowAllowlist>("/api/computer/allowlist", {
        enabled: allowlist.enabled,
        windows: allowWindows
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean),
        apps: allowApps
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean),
      });
      setAllowlist(saved);
      toast({ tone: "success", title: "Allowlist de janelas salva" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao salvar allowlist",
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
      <SandboxPolicyCard
        sandbox={sandbox}
        setSandbox={setSandbox}
        commands={commands}
        paths={paths}
        onCommandsChange={setCommands}
        onPathsChange={setPaths}
        busy={busy}
        onSave={() => void saveSandbox()}
      />
      <ComputerUseCard
        capabilities={capabilities}
        sessions={sessions}
        allowlist={allowlist}
        allowWindows={allowWindows}
        allowApps={allowApps}
        onAllowlistChange={setAllowlist}
        onAllowWindowsChange={setAllowWindows}
        onAllowAppsChange={setAllowApps}
        requireApproval={requireApproval}
        onRequireApprovalChange={setRequireApproval}
        isolated={isolated}
        onIsolatedChange={setIsolated}
        busy={busy}
        action={action}
        onActionChange={setAction}
        active={active}
        preview={preview}
        onCreate={() => void createSession()}
        onSaveAllowlist={() => void saveAllowlist()}
        onSessionAction={(id, kind) => void sessionAction(id, kind)}
        onCapture={(id) => void capture(id)}
        onSendAction={(session) => void sendAction(session)}
      />
    </div>
  );
}
