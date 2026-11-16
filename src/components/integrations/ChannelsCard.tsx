import { useCallback, useEffect, useState } from "react";
import { KeyRound, Loader2, Plus, Radio, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";

interface ChannelStatus {
  channel: string;
  routes: number;
  webhook_token_set: boolean;
  webhook_path: string;
  connector: string;
}

interface ChannelRoute {
  id: string;
  channel: string;
  identity: string;
  agent: string;
  permission_profile: string;
  trust_level: string;
  created_at: string;
}

interface WhatsAppConfig {
  enabled: boolean;
  provider: string;
  phone_number: string;
  phone_number_id: string;
  business_account_id: string;
  api_version: string;
  workspace_id?: string | null;
  notes: string;
}

const CHANNELS = ["whatsapp", "telegram", "signal", "email", "web", "api"];
const TRUST = ["untrusted", "internal", "trusted"];
const PROFILES = ["read_only", "default", "full"];

const inputClass =
  "h-8 rounded border border-zinc-700 bg-zinc-950 px-2 text-[12.5px] text-zinc-200";

export function ChannelsCard({ workspaceId }: { workspaceId: string }) {
  const [statuses, setStatuses] = useState<ChannelStatus[]>([]);
  const [routes, setRoutes] = useState<ChannelRoute[]>([]);
  const [whatsapp, setWhatsapp] = useState<WhatsAppConfig>({
    enabled: false,
    provider: "structure",
    phone_number: "",
    phone_number_id: "",
    business_account_id: "",
    api_version: "v21.0",
    workspace_id: null,
    notes: "",
  });
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
  const [form, setForm] = useState({
    channel: "telegram",
    identity: "*",
    agent: "build",
    trust: "untrusted",
    profile: "read_only",
  });

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
          <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
            {statuses.map((status) => (
              <div
                key={status.channel}
                className="space-y-1 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono text-[12.5px] text-zinc-200">
                    {status.channel}
                  </span>
                  <Badge variant={status.webhook_token_set ? "green" : "default"}>
                    {status.webhook_token_set ? "token" : "sem token"}
                  </Badge>
                </div>
                <p className="text-[11.5px] text-zinc-500">
                  {status.connector} · {status.routes} rota(s)
                </p>
                <div className="flex items-center justify-between gap-2">
                  <code className="truncate font-mono text-[10.5px] text-zinc-600">
                    {status.webhook_path}
                  </code>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-6 px-2 text-[11px]"
                    disabled={busy === `token:${status.channel}`}
                    onClick={() => void generateToken(status.channel)}
                  >
                    <KeyRound size={10} />
                    Token
                  </Button>
                </div>
              </div>
            ))}
          </div>

          {revealed ? (
            <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-[12px] text-amber-200">
              Token de <strong>{revealed.channel}</strong> (copie agora):{" "}
              <code className="break-all font-mono">{revealed.token}</code>
              <p className="mt-1 text-[11.5px] text-amber-200/70">
                Envie em <code className="font-mono">x-opendesk-channel-token</code>.
              </p>
            </div>
          ) : null}

          <div className="grid gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5 sm:grid-cols-2">
            <label className="flex items-center gap-2 text-[13px] text-zinc-300">
              <input
                type="checkbox"
                checked={whatsapp.enabled}
                onChange={(event) =>
                  setWhatsapp({ ...whatsapp, enabled: event.target.checked })
                }
                className="h-4 w-4 accent-blue-500"
              />
              WhatsApp (estrutura)
              {tokenSet ? <Badge variant="green">webhook</Badge> : null}
            </label>
            <select
              value={whatsapp.provider}
              onChange={(event) =>
                setWhatsapp({ ...whatsapp, provider: event.target.value })
              }
              className={inputClass}
            >
              <option value="structure">structure (sem conector)</option>
              <option value="cloud-api">cloud-api (planejado)</option>
              <option value="baileys">baileys (planejado)</option>
            </select>
            <input
              value={whatsapp.phone_number}
              onChange={(event) =>
                setWhatsapp({ ...whatsapp, phone_number: event.target.value })
              }
              placeholder="+55 11 99999-0000"
              className={inputClass}
            />
            <input
              value={whatsapp.phone_number_id}
              onChange={(event) =>
                setWhatsapp({ ...whatsapp, phone_number_id: event.target.value })
              }
              placeholder="phone_number_id (Meta)"
              className={inputClass}
            />
            <input
              value={whatsapp.business_account_id}
              onChange={(event) =>
                setWhatsapp({ ...whatsapp, business_account_id: event.target.value })
              }
              placeholder="business account id (opcional)"
              className={inputClass}
            />
            <input
              value={whatsapp.api_version}
              onChange={(event) =>
                setWhatsapp({ ...whatsapp, api_version: event.target.value })
              }
              placeholder="v21.0"
              className={inputClass}
            />
            <input
              value={whatsapp.notes}
              onChange={(event) =>
                setWhatsapp({ ...whatsapp, notes: event.target.value })
              }
              placeholder="observações do número/conta"
              className={inputClass}
            />
            <input
              type="password"
              value={accessToken}
              onChange={(event) => setAccessToken(event.target.value)}
              placeholder={accessTokenSet ? "token salvo (deixe vazio para manter)" : "access token (Meta)"}
              className={inputClass}
              autoComplete="new-password"
            />
            <input
              type="password"
              value={appSecret}
              onChange={(event) => setAppSecret(event.target.value)}
              placeholder={appSecretSet ? "app secret salvo (deixe vazio para manter)" : "app secret (assinatura)"}
              className={inputClass}
              autoComplete="new-password"
            />
            <div className="sm:col-span-2 flex items-center justify-between gap-2">
              <p className="text-[11.5px] text-zinc-500">
                O conector real (Cloud API/Baileys) fica para a onda de canais
                externos. Rotas e webhook já funcionam.
              </p>
              <div className="flex items-center gap-1.5">
                {accessTokenSet && appSecretSet ? (
                  <Badge variant="green">cloud-api pronta</Badge>
                ) : (
                  <Badge variant="default">faltam credenciais</Badge>
                )}
                <Button
                  size="sm"
                  className="h-7"
                  disabled={busy === "whatsapp"}
                  onClick={() => void saveWhatsApp()}
                >
                  Salvar
                </Button>
              </div>
            </div>
            <div className="sm:col-span-2 flex flex-wrap items-end gap-2 border-t border-zinc-800/60 pt-2">
              <input
                value={sendTo}
                onChange={(event) => setSendTo(event.target.value)}
                placeholder="destinatário (5511999...)"
                className={`${inputClass} w-44 font-mono`}
              />
              <input
                value={sendText}
                onChange={(event) => setSendText(event.target.value)}
                placeholder="texto de teste"
                className={`${inputClass} flex-1`}
              />
              <Button
                variant="outline"
                size="sm"
                className="h-8"
                disabled={busy === "send" || !accessTokenSet || !whatsapp.phone_number_id}
                onClick={() => void sendWhatsApp()}
              >
                Enviar teste
              </Button>
            </div>
          </div>

          <div className="space-y-2">
            <div className="flex flex-wrap items-end gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
              <select
                value={form.channel}
                onChange={(event) => setForm({ ...form, channel: event.target.value })}
                className={inputClass}
              >
                {CHANNELS.map((channel) => (
                  <option key={channel} value={channel}>
                    {channel}
                  </option>
                ))}
              </select>
              <input
                value={form.identity}
                onChange={(event) => setForm({ ...form, identity: event.target.value })}
                placeholder="identidade ou *"
                className={`${inputClass} w-40 font-mono`}
              />
              <input
                value={form.agent}
                onChange={(event) => setForm({ ...form, agent: event.target.value })}
                placeholder="agente"
                className={`${inputClass} w-28`}
              />
              <select
                value={form.trust}
                onChange={(event) => setForm({ ...form, trust: event.target.value })}
                className={inputClass}
              >
                {TRUST.map((trust) => (
                  <option key={trust} value={trust}>
                    {trust}
                  </option>
                ))}
              </select>
              <select
                value={form.profile}
                onChange={(event) => setForm({ ...form, profile: event.target.value })}
                className={inputClass}
              >
                {PROFILES.map((profile) => (
                  <option key={profile} value={profile}>
                    {profile}
                  </option>
                ))}
              </select>
              <Button
                size="sm"
                className="h-8"
                disabled={busy === "route"}
                onClick={() => void addRoute()}
              >
                <Plus size={12} />
                Rota
              </Button>
            </div>

            {loading ? (
              <div className="flex items-center gap-2 py-3 text-[12.5px] text-zinc-500">
                <Loader2 size={13} className="animate-spin" />
                carregando rotas…
              </div>
            ) : routes.length === 0 ? (
              <EmptyState
                title="Nenhuma rota"
                description="Sem rota, mensagens recebidas são recusadas. Crie ao menos uma rota por canal."
              />
            ) : (
              <div className="overflow-hidden rounded-lg border border-zinc-800/70">
                <table className="w-full text-left text-[12.5px]">
                  <thead className="bg-zinc-900/60 text-[11px] uppercase tracking-[0.08em] text-zinc-500">
                    <tr>
                      <th className="px-3 py-1.5">canal</th>
                      <th className="px-3 py-1.5">identidade</th>
                      <th className="px-3 py-1.5">agente</th>
                      <th className="px-3 py-1.5">trust</th>
                      <th className="px-3 py-1.5">perfil</th>
                      <th className="px-3 py-1.5" />
                    </tr>
                  </thead>
                  <tbody>
                    {routes.map((route) => (
                      <tr key={route.id} className="border-t border-zinc-800/60">
                        <td className="px-3 py-1.5 font-mono text-zinc-300">
                          {route.channel}
                        </td>
                        <td className="px-3 py-1.5 font-mono text-zinc-400">
                          {route.identity}
                        </td>
                        <td className="px-3 py-1.5 text-zinc-300">{route.agent}</td>
                        <td className="px-3 py-1.5">
                          <Badge
                            variant={
                              route.trust_level === "trusted"
                                ? "green"
                                : route.trust_level === "internal"
                                  ? "blue"
                                  : "default"
                            }
                          >
                            {route.trust_level}
                          </Badge>
                        </td>
                        <td className="px-3 py-1.5 text-zinc-400">
                          {route.permission_profile}
                        </td>
                        <td className="px-3 py-1.5 text-right">
                          <Button
                            variant="ghost"
                            size="sm"
                            className="h-6 px-2"
                            disabled={busy === `route:${route.id}`}
                            onClick={() => void removeRoute(route.id)}
                          >
                            <Trash2 size={11} />
                          </Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
