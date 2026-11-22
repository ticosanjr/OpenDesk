import { useCallback, useEffect, useState } from "react";
import { Radio } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";
import { ChannelStatusGrid } from "@/components/integrations/channels/ChannelStatusGrid";
import { RoutesSection, type RouteForm } from "@/components/integrations/channels/RoutesSection";
import { WhatsAppSection } from "@/components/integrations/channels/WhatsAppSection";
import type {
  ChannelRoute,
  ChannelStatus,
  WhatsAppConfig,
} from "@/components/integrations/channels/types";

const EMPTY_WHATSAPP: WhatsAppConfig = {
  enabled: false,
  provider: "structure",
  phone_number: "",
  phone_number_id: "",
  business_account_id: "",
  api_version: "v21.0",
  workspace_id: null,
  notes: "",
};

const EMPTY_FORM: RouteForm = {
  channel: "telegram",
  identity: "*",
  agent: "build",
  trust: "untrusted",
  profile: "read_only",
};

export function ChannelsCard({ workspaceId }: { workspaceId: string }) {
  const [statuses, setStatuses] = useState<ChannelStatus[]>([]);
  const [routes, setRoutes] = useState<ChannelRoute[]>([]);
  const [whatsapp, setWhatsapp] = useState<WhatsAppConfig>(EMPTY_WHATSAPP);
  const [tokenSet, setTokenSet] = useState(false);
  const [accessTokenSet, setAccessTokenSet] = useState(false);
  const [appSecretSet, setAppSecretSet] = useState(false);
  const [accessToken, setAccessToken] = useState("");
  const [appSecret, setAppSecret] = useState("");
  const [sendTo, setSendTo] = useState("");
  const [sendText, setSendText] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [revealed, setRevealed] = useState<{ channel: string; token: string } | null>(
    null,
  );
  const [form, setForm] = useState<RouteForm>(EMPTY_FORM);

  const query = `?workspace_id=${encodeURIComponent(workspaceId)}`;

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      const [status, list, config] = await Promise.all([
        apiGet<ChannelStatus[]>(`/api/channels/status${query}`),
        apiGet<ChannelRoute[]>(`/api/channels/routes${query}`),
        apiGet<{
          config: WhatsAppConfig;
          webhook_token_set: boolean;
          access_token_set: boolean;
          app_secret_set: boolean;
          connected: boolean;
        }>("/api/channels/whatsapp"),
      ]);
      setStatuses(status);
      setRoutes(list);
      setWhatsapp(config.config);
      setTokenSet(config.webhook_token_set);
      setAccessTokenSet(config.access_token_set);
      setAppSecretSet(config.app_secret_set);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar os canais",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, [workspaceId, query]);

  useEffect(() => {
    void load();
  }, [load]);

  async function saveWhatsApp() {
    setBusy("whatsapp");
    try {
      await apiPut("/api/channels/whatsapp", {
        ...whatsapp,
        access_token: accessToken.trim() || undefined,
        app_secret: appSecret.trim() || undefined,
      });
      setAccessToken("");
      setAppSecret("");
      toast({
        tone: "success",
        title: "Configuração de WhatsApp salva",
        description:
          whatsapp.provider === "cloud-api"
            ? "Cloud API configurada; use o teste de envio."
            : "Estrutura pronta para configurar o provider.",
      });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao salvar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function sendWhatsApp() {
    if (!sendTo.trim() || !sendText.trim()) {
      toast({ tone: "error", title: "Informe destinatário e texto" });
      return;
    }
    setBusy("send");
    try {
      const result = await apiPost<{ message_id?: string }>(
        "/api/channels/whatsapp/send",
        { to: sendTo.trim(), text: sendText.trim() },
      );
      setSendText("");
      toast({
        tone: "success",
        title: "Mensagem enviada",
        description: result.message_id ?? "Meta aceitou o envio.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha no envio",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function generateToken(channel: string) {
    setBusy(`token:${channel}`);
    try {
      const result = await apiPost<{ token: string }>(
        `/api/channels/${channel}/webhook-token`,
      );
      setRevealed({ channel, token: result.token });
      await load();
      toast({
        tone: "success",
        title: "Token gerado",
        description: "Copie agora; ele não é exibido novamente.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao gerar token",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function addRoute() {
    setBusy("route");
    try {
      await apiPost(`/api/channels/routes${query}`, {
        channel: form.channel,
        identity: form.identity.trim() || "*",
        agent: form.agent.trim() || "build",
        permission_profile: form.profile,
        trust_level: form.trust,
      });
      await load();
      toast({ tone: "success", title: "Rota salva" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao salvar a rota",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function removeRoute(id: string) {
    setBusy(`route:${id}`);
    try {
      await apiDelete(`/api/channels/routes/${id}`);
      await load();
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="space-y-4">
      <Card id="integrations-channels">
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Radio size={15} className="text-zinc-500" />
              Canais e confiança
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Cada identidade recebe um agente e um nível de confiança. Mensagens
              não confiáveis viram ticket; confiáveis viram tarefa. WhatsApp está
              estruturado, sem conector ativo.
            </p>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <ChannelStatusGrid
            statuses={statuses}
            revealed={revealed}
            busy={busy}
            onGenerateToken={(channel) => void generateToken(channel)}
          />
          <WhatsAppSection
            whatsapp={whatsapp}
            onChange={setWhatsapp}
            tokenSet={tokenSet}
            accessTokenSet={accessTokenSet}
            appSecretSet={appSecretSet}
            accessToken={accessToken}
            onAccessTokenChange={setAccessToken}
            appSecret={appSecret}
            onAppSecretChange={setAppSecret}
            sendTo={sendTo}
            onSendToChange={setSendTo}
            sendText={sendText}
            onSendTextChange={setSendText}
            busy={busy}
            onSave={() => void saveWhatsApp()}
            onSend={() => void sendWhatsApp()}
          />
          <RoutesSection
            routes={routes}
            loading={loading}
            busy={busy}
            form={form}
            onFormChange={setForm}
            onAdd={() => void addRoute()}
            onRemove={(id) => void removeRoute(id)}
          />
        </CardContent>
      </Card>
    </div>
  );
}
