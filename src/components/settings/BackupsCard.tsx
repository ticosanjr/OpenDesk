import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Archive,
  Download,
  Loader2,
  Plus,
  RefreshCw,
  RotateCcw,
  Trash2,
  Upload,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { apiBlob, apiDelete, apiGet, apiPost } from "@/lib/api";
import { pickFolder, slugifyWorkspace } from "@/lib/folder";
import { downloadBlob } from "@/lib/download";
import { relativeTime } from "@/lib/format";
import type {
  BackupOptions,
  RestoreMode,
  RestoreReport,
  Workspace,
  WorkspaceBackup,
} from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import { RestoreBackupModal } from "@/components/settings/backups/RestoreBackupModal";
import {
  countsTitle,
  databaseRecords,
  fileToBase64,
  megabytes,
} from "@/components/settings/backups/backup-format";
import { cn } from "@/lib/utils";

export function BackupsCard() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const refresh = useAppStore((state) => state.refresh);
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const [backups, setBackups] = useState<WorkspaceBackup[]>([]);
  const [loading, setLoading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const importRef = useRef<HTMLInputElement | null>(null);
  const [restoring, setRestoring] = useState<WorkspaceBackup | null>(null);
  const [restoreName, setRestoreName] = useState("");
  const [restorePath, setRestorePath] = useState("");
  const [restoreMode, setRestoreMode] =
    useState<RestoreMode>("new_workspace");
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [defaultsRoot, setDefaultsRoot] = useState("");
  const [includeLogs, setIncludeLogs] = useState(true);
  const [backupPassphrase, setBackupPassphrase] = useState("");
  const [restorePassphrase, setRestorePassphrase] = useState("");

  const query = useMemo(
    () =>
      activeWorkspaceId
        ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}`
        : "",
    [activeWorkspaceId],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setBackups([]);
      return;
    }
    setLoading(true);
    try {
      setBackups(
        await apiGet<WorkspaceBackup[]>(`/api/backups${query}`),
      );
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível listar backups",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    apiGet<{ workspaces_root: string }>("/api/app/defaults")
      .then((value) => setDefaultsRoot(value.workspaces_root))
      .catch(() => setDefaultsRoot(""));
  }, []);

  async function createBackup() {
    if (!activeWorkspaceId) return;
    setCreating(true);
    try {
      const options: BackupOptions = {
        include_logs: includeLogs,
        passphrase: backupPassphrase.trim() || undefined,
      };
      const backup = await apiPost<WorkspaceBackup>(
        `/api/backups${query}`,
        options,
      );
      await load();
      toast({
        tone: "success",
        title: "Backup criado",
        description: `${backup.files} arquivos · ${megabytes(backup.size_bytes)}`,
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível criar o backup",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setCreating(false);
    }
  }

  function openRestore(backup: WorkspaceBackup) {
    const name = `${workspace?.name ?? backup.workspace_name} (restaurado)`;
    setRestoring(backup);
    setRestoreName(name);
    const root = defaultsRoot || "";
    setRestorePath(root ? `${root}/${slugifyWorkspace(name)}` : "");
    setRestoreError(null);
  }

  async function chooseTarget() {
    const selected = await pickFolder(
      "Pasta de destino do workspace restaurado",
      restorePath || defaultsRoot || undefined,
    );
    if (selected) setRestorePath(selected);
  }

  async function restore() {
    if (!restoring) return;
    if (restoreMode === "new_workspace" && !restorePath.trim()) {
      setRestoreError("Informe a pasta de destino.");
      return;
    }
    if (restoreMode === "in_place") {
      // An in-place restore writes files and rows, so force the operator to
      // acknowledge the destructive mode explicitly.
      const confirmed = await confirmAction({
        title: "Substituir o workspace atual?",
        description:
          "As tarefas, runs, memórias e arquivos atuais serão substituídos pelo conteúdo do backup.",
        confirmLabel: "Substituir",
        tone: "destructive",
      });
      if (!confirmed) return;
    }
    setBusy(`restore-${restoring.id}`);
    setRestoreError(null);
    try {
      const created = await apiPost<
        Workspace & { restore_report?: RestoreReport }
      >(`/api/backups/${restoring.id}/restore${query}`, {
        target_path: restorePath.trim(),
        name: restoreName.trim(),
        mode: restoreMode,
        passphrase: restorePassphrase.trim() || undefined,
      });
      await load();
      await refresh();
      setRestoring(null);
      const missing = created.restore_report?.missing_secrets.length ?? 0;
      toast({
        tone: missing > 0 ? "warning" : "success",
        title: "Workspace restaurado",
        description:
          missing > 0
            ? `${created.name} · ${missing} segredo(s) precisam ser reenviados`
            : created.name,
        href: "/settings?section=workspaces",
      });
    } catch (cause) {
      setRestoreError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function importBackup(file: File) {
    if (!activeWorkspaceId) return;
    setBusy("import");
    try {
      const content = await fileToBase64(file);
      await apiPost<WorkspaceBackup>(`/api/backups/import${query}`, {
        file_name: file.name,
        content_base64: content,
      });
      await load();
      toast({
        tone: "success",
        title: "Backup importado",
        description: file.name,
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível importar o backup",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
      if (importRef.current) importRef.current.value = "";
    }
  }

  async function download(backup: WorkspaceBackup) {
    setBusy(`download-${backup.id}`);
    try {
      const blob = await apiBlob(
        `/api/backups/${backup.id}/download${query}`,
      );
      downloadBlob(blob, backup.file_name);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível baixar o backup",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function remove(backup: WorkspaceBackup) {
    const confirmed = await confirmAction({
      title: `Excluir o backup de ${backup.workspace_name}?`,
      description: `O arquivo ${backup.file_name} será removido.`,
      confirmLabel: "Excluir backup",
      tone: "destructive",
    });
    if (!confirmed) return;
    setBusy(`delete-${backup.id}`);
    try {
      await apiDelete(`/api/backups/${backup.id}${query}`);
      await load();
      toast({ tone: "success", title: "Backup excluído" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível excluir",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  if (!activeWorkspaceId) {
    return (
      <EmptyState
        icon={Archive}
        title="Nenhum workspace ativo"
        description="Selecione um workspace para criar e restaurar backups."
      />
    );
  }

  return (
    <>
      <Card id="settings-backups">
        <CardHeader className="flex flex-row items-start justify-between gap-3">
          <div>
            <CardTitle className="flex items-center gap-2">
              <Archive size={15} className="text-zinc-400" />
              Backup do workspace
            </CardTitle>
            <p className="mt-1 text-xs leading-relaxed text-zinc-500">
              Inclui arquivos, entradas, saídas e `.opencode/`, excluindo
              `node_modules`, `.git` e caches. O arquivo é verificado por SHA-256
              e pode ser restaurado como um novo workspace.
            </p>
          </div>
          <div className="flex shrink-0 flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => importRef.current?.click()}
              disabled={busy === "import"}
            >
              {busy === "import" ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Upload size={13} />
              )}
              Importar .zip
            </Button>
            <input
              ref={importRef}
              type="file"
              accept="application/zip,.zip"
              className="hidden"
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (file) void importBackup(file);
              }}
            />
            <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
            <Button size="sm" onClick={() => void createBackup()} disabled={creating}>
              {creating ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Plus size={13} />
              )}
              Criar backup
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex flex-wrap items-center gap-2 text-[12px] text-zinc-500">
            <Badge variant="default">{workspace?.name}</Badge>
            <span>{backups.length} backup(s)</span>
            {defaultsRoot ? (
              <span className="font-mono text-zinc-500">destino padrão: {defaultsRoot}</span>
            ) : null}
          </div>
          <div className="flex flex-wrap items-end gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
            <label className="flex items-center gap-2 text-[12.5px] text-zinc-400">
              <input
                type="checkbox"
                checked={includeLogs}
                onChange={(event) => setIncludeLogs(event.target.checked)}
                className="h-3.5 w-3.5 accent-blue-500"
              />
              Incluir logs de execução
            </label>
            <label className="min-w-[220px] flex-1">
              <span className={labelClass}>Senha para segredos (opcional)</span>
              <input
                type="password"
                value={backupPassphrase}
                onChange={(event) => setBackupPassphrase(event.target.value)}
                placeholder="protege SMTP, tokens e Vault"
                className={cn(inputClass, "h-8 text-[12px]")}
              />
            </label>
          </div>
          {loading && backups.length === 0 ? (
            <LoadingState rows={2} label="Carregando backups…" />
          ) : backups.length === 0 ? (
            <EmptyState
              icon={Archive}
              title="Nenhum backup ainda"
              description="Crie o primeiro backup completo deste workspace."
            />
          ) : (
            <div className="space-y-1.5">
              {backups.map((backup) => (
                <div
                  key={backup.id}
                  className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-1.5">
                      <p className="truncate font-mono text-[12px] text-zinc-300">
                        {backup.file_name}
                      </p>
                      <Badge variant="blue">{megabytes(backup.size_bytes)}</Badge>
                      <Badge variant="default">{backup.files} arquivos</Badge>
                      <Badge
                        variant="purple"
                        title={countsTitle(backup.counts)}
                      >
                        {databaseRecords(backup.counts)} registros
                      </Badge>
                      <Badge variant="default">v{backup.version}</Badge>
                    </div>
                    <p className="mt-0.5 text-[11.5px] text-zinc-500">
                      {relativeTime(backup.created_at)} · SHA-256{" "}
                      <span className="font-mono">{backup.sha256.slice(0, 12)}…</span>
                    </p>
                  </div>
                  <div className="flex shrink-0 gap-1.5">
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7"
                      disabled={busy === `download-${backup.id}`}
                      onClick={() => void download(backup)}
                    >
                      <Download size={11} />
                      Baixar
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7"
                      disabled={busy === `restore-${backup.id}`}
                      onClick={() => openRestore(backup)}
                    >
                      <RotateCcw size={11} />
                      Restaurar
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-7 text-zinc-500 hover:text-red-300"
                      disabled={busy === `delete-${backup.id}`}
                      onClick={() => void remove(backup)}
                      aria-label={`Excluir backup ${backup.file_name}`}
                    >
                      <Trash2 size={11} />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <RestoreBackupModal
        backup={restoring}
        mode={restoreMode}
        name={restoreName}
        path={restorePath}
        passphrase={restorePassphrase}
        error={restoreError}
        restoring={busy?.startsWith("restore-") ?? false}
        onMode={setRestoreMode}
        onName={setRestoreName}
        onPath={setRestorePath}
        onPassphrase={setRestorePassphrase}
        onChooseTarget={() => void chooseTarget()}
        onClose={() => setRestoring(null)}
        onRestore={() => void restore()}
      />
    </>
  );
}
