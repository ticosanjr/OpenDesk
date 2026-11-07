import { useEffect, useState } from "react";
import { Bell, Check, Loader2 } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { apiGet, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";

export function NotificationCard() {
  const [native, setNative] = useState(true);
  const [artifacts, setArtifacts] = useState(true);
  const [closeToTray, setCloseToTray] = useState(true);
  const [permissionTimeout, setPermissionTimeout] = useState(10);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setNative(localStorage.getItem("opendesk.native_notifications") !== "false");
    setArtifacts(
      localStorage.getItem("opendesk.artifact_notifications") !== "false",
    );
    void apiGet<Array<{ key: string; value_json: string }>>("/api/settings")
      .then((settings) => {
        const timeout = settings.find((item) => item.key === "permission_timeout_minutes");
        if (timeout) {
          try {
            setPermissionTimeout(Number(JSON.parse(timeout.value_json)) || 10);
          } catch {
            setPermissionTimeout(10);
          }
        }
        const entry = settings.find((item) => item.key === "close_to_tray");
        if (!entry) return true;
        try {
          return JSON.parse(entry.value_json) === true;
        } catch {
          return false;
        }
      })
      .then((value) => setCloseToTray(value))
      .catch(() => setCloseToTray(true))
      .finally(() => setLoading(false));
  }, []);

  async function saveCloseToTray(value: boolean) {
    setSaving(true);
    try {
      await apiPut("/api/settings/close_to_tray", value);
      setCloseToTray(value);
      toast({
        tone: "success",
        title: value ? "Fechar janela: bandeja" : "Fechar janela: sair",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card id="settings-notifications" className="scroll-mt-14">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Bell size={15} className="text-zinc-500" />
            Notificações e fechamento
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Controle notificações nativas, avisos de artefatos e o que acontece ao fechar a janela.
          </p>
        </div>
        {saving ? (
          <Loader2 size={14} className="animate-spin text-zinc-500" />
        ) : (
          <Badge variant="green">
            <Check size={10} />
            local
          </Badge>
        )}
      </CardHeader>
      <CardContent className="space-y-3">
        <label className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
          <span>
            <span className="block text-[12px] font-medium text-zinc-200">
              Notificações nativas
            </span>
            <span className="text-[11px] text-zinc-500">
              Só quando a janela estiver oculta ou sem foco.
            </span>
          </span>
          <input
            type="checkbox"
            checked={native}
            disabled={loading}
            onChange={(event) => {
              const value = event.target.checked;
              setNative(value);
              localStorage.setItem("opendesk.native_notifications", String(value));
            }}
            className="h-4 w-4 accent-blue-500"
          />
        </label>
        <label className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
          <span>
            <span className="block text-[12px] font-medium text-zinc-200">
              Avisar sobre artefatos
            </span>
            <span className="text-[11px] text-zinc-500">
              No máximo um aviso por execução.
            </span>
          </span>
          <input
            type="checkbox"
            checked={artifacts}
            disabled={loading}
            onChange={(event) => {
              const value = event.target.checked;
              setArtifacts(value);
              localStorage.setItem("opendesk.artifact_notifications", String(value));
            }}
            className="h-4 w-4 accent-blue-500"
          />
        </label>
        <label className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
          <span>
            <span className="block text-[12px] font-medium text-zinc-200">
              Timeout de permissão humana
            </span>
            <span className="text-[11px] text-zinc-500">
              Após esse tempo sem resposta, a tarefa é bloqueada e o worker é liberado.
            </span>
          </span>
          <input
            type="number"
            min={1}
            max={60}
            value={permissionTimeout}
            disabled={loading || saving}
            onChange={(event) => setPermissionTimeout(Number(event.target.value))}
            onBlur={() => void apiPut("/api/settings/permission_timeout_minutes", permissionTimeout)}
            className="h-7 w-16 rounded border border-zinc-700 bg-zinc-950 px-2 text-right font-mono text-[11px] text-zinc-200"
          />
        </label>
        <label className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
          <span>
            <span className="block text-[12px] font-medium text-zinc-200">
              Fechar janela mantendo na bandeja
            </span>
            <span className="text-[11px] text-zinc-500">
              Padrão: ligado. Desligado, o “X” encerra o app e para as notificações.
            </span>
          </span>
          <input
            type="checkbox"
            checked={closeToTray}
            disabled={loading || saving}
            onChange={(event) => void saveCloseToTray(event.target.checked)}
            className="h-4 w-4 accent-blue-500"
          />
        </label>
      </CardContent>
    </Card>
  );
}
