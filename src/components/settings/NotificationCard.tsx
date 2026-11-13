import { useEffect, useState } from "react";
import { Bell, Check, Loader2, RotateCcw } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { apiGet, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";
import {
  notificationEnabled,
  resetNotificationPreferences,
  setNotificationEnabled,
  type NotificationKind,
} from "@/lib/notification-policy";

export function NotificationCard() {
  const [native, setNative] = useState(true);
  const [runFinished, setRunFinished] = useState(true);
  const [runFailed, setRunFailed] = useState(true);
  const [interactions, setInteractions] = useState(true);
  const [artifacts, setArtifacts] = useState(false);
  const [scheduleSkips, setScheduleSkips] = useState(false);
  const [closeToTray, setCloseToTray] = useState(true);
  const [permissionTimeout, setPermissionTimeout] = useState(10);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setNative(localStorage.getItem("opendesk.native_notifications") !== "false");
    setRunFinished(notificationEnabled("run_finished"));
    setRunFailed(notificationEnabled("run_failed"));
    setInteractions(notificationEnabled("interactions"));
    setArtifacts(notificationEnabled("artifacts"));
    setScheduleSkips(notificationEnabled("schedule_skips"));
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

  function togglePreference(
    kind: NotificationKind,
    value: boolean,
    setter: (value: boolean) => void,
  ) {
    setNotificationEnabled(kind, value);
    setter(value);
  }

  function restoreMinimalDefaults() {
    resetNotificationPreferences();
    setRunFinished(true);
    setRunFailed(true);
    setInteractions(true);
    setArtifacts(false);
    setScheduleSkips(false);
    toast({
      tone: "success",
      title: "Padrões mínimos restaurados",
      description: "Só resultados, falhas e interações continuam avisando.",
    });
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
            <span className="block text-[13.5px] font-medium text-zinc-200">
              Notificações nativas
            </span>
            <span className="text-[13px] text-zinc-500">
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
        <NotificationToggle
          label="Fim de execução"
          description="Avisa quando um run termina com sucesso."
          checked={runFinished}
          disabled={loading}
          onCheckedChange={(value) =>
            togglePreference("run_finished", value, setRunFinished)
          }
        />
        <NotificationToggle
          label="Falhas e bloqueios"
          description="Avisa quando um run falha, é cancelado ou bloqueia uma tarefa."
          checked={runFailed}
          disabled={loading}
          onCheckedChange={(value) =>
            togglePreference("run_failed", value, setRunFailed)
          }
        />
        <NotificationToggle
          label="Perguntas e permissões"
          description="Avisa quando um agente precisa da sua resposta ou autorização."
          checked={interactions}
          disabled={loading}
          onCheckedChange={(value) =>
            togglePreference("interactions", value, setInteractions)
          }
        />
        <NotificationToggle
          label="Artefatos"
          description="Opcional: no máximo um aviso por execução. Desligado por padrão."
          checked={artifacts}
          disabled={loading}
          onCheckedChange={(value) =>
            togglePreference("artifacts", value, setArtifacts)
          }
        />
        <NotificationToggle
          label="Rotinas puladas"
          description="Opcional: avisa quando uma rotina é pulada por motor offline ou atraso."
          checked={scheduleSkips}
          disabled={loading}
          onCheckedChange={(value) =>
            togglePreference("schedule_skips", value, setScheduleSkips)
          }
        />
        <label className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
          <span>
            <span className="block text-[13.5px] font-medium text-zinc-200">
              Timeout de permissão humana
            </span>
            <span className="text-[13px] text-zinc-500">
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
            className="h-7 w-16 rounded border border-zinc-700 bg-zinc-950 px-2 text-right font-mono text-[13px] text-zinc-200"
          />
        </label>
        <div className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
          <p className="text-[12px] leading-snug text-zinc-500">
            Movimentações de Kanban nunca geram notificação. O padrão é avisar
            só o que exige atenção.
          </p>
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={restoreMinimalDefaults}
          >
            <RotateCcw size={11} />
            Restaurar padrões mínimos
          </Button>
        </div>
        <label className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
          <span>
            <span className="block text-[13.5px] font-medium text-zinc-200">
              Fechar janela mantendo na bandeja
            </span>
            <span className="text-[13px] text-zinc-500">
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


function NotificationToggle({
  label,
  description,
  checked,
  disabled,
  onCheckedChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (value: boolean) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
      <span>
        <span className="block text-[13.5px] font-medium text-zinc-200">{label}</span>
        <span className="text-[13px] text-zinc-500">{description}</span>
      </span>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onCheckedChange(event.target.checked)}
        className="h-4 w-4 accent-blue-500"
      />
    </label>
  );
}
