import { useEffect, useState } from "react";
import { BellRing, Loader2, Send } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  currentPushSubscription,
  pushSupported,
  sendTestPush,
  subscribeToPush,
  unsubscribeFromPush,
} from "@/lib/push";
import { toast } from "@/store/toast-store";

/**
 * Web Push is only meaningful outside Tauri: the desktop app already has native
 * notifications. The panel is hidden when the browser lacks Push support.
 */
export function PushNotificationPanel() {
  const [supported] = useState(() => pushSupported());
  const [subscribed, setSubscribed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [checking, setChecking] = useState(supported);

  useEffect(() => {
    if (!supported) return;
    let cancelled = false;
    void currentPushSubscription()
      .then((subscription) => {
        if (!cancelled) setSubscribed(Boolean(subscription));
      })
      .catch(() => {
        if (!cancelled) setSubscribed(false);
      })
      .finally(() => {
        if (!cancelled) setChecking(false);
      });
    return () => {
      cancelled = true;
    };
  }, [supported]);

  if (!supported) return null;

  async function enable() {
    setBusy(true);
    try {
      await subscribeToPush();
      setSubscribed(true);
      toast({
        tone: "success",
        title: "Push ativado",
        description: "Este dispositivo recebe avisos mesmo com a aba fechada.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível ativar o push",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(false);
    }
  }

  async function disable() {
    setBusy(true);
    try {
      await unsubscribeFromPush();
      setSubscribed(false);
      toast({ tone: "success", title: "Push desativado neste dispositivo" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível desativar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(false);
    }
  }

  async function test() {
    setBusy(true);
    try {
      const delivered = await sendTestPush();
      toast({
        tone: "success",
        title: "Teste enviado",
        description: `${delivered} dispositivo(s) receberam a notificação.`,
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "O teste não chegou",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
      <div className="flex items-start justify-between gap-3">
        <span>
          <span className="flex items-center gap-1.5 text-[13.5px] font-medium text-zinc-200">
            <BellRing size={13} className="text-zinc-500" />
            Push no navegador
            {subscribed ? (
              <Badge variant="green" className="ml-1">
                ativo
              </Badge>
            ) : null}
          </span>
          <span className="text-[13px] text-zinc-500">
            Receba avisos de execuções e aprovações no PWA, mesmo sem a aba aberta.
          </span>
        </span>
        {checking ? (
          <Loader2 size={14} className="animate-spin text-zinc-500" />
        ) : (
          <div className="flex shrink-0 items-center gap-1.5">
            {subscribed ? (
              <>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7"
                  disabled={busy}
                  onClick={() => void test()}
                >
                  <Send size={11} />
                  Testar
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7"
                  disabled={busy}
                  onClick={() => void disable()}
                >
                  Desativar
                </Button>
              </>
            ) : (
              <Button
                size="sm"
                className="h-7"
                disabled={busy}
                onClick={() => void enable()}
              >
                Ativar
              </Button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
