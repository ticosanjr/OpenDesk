import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  CalendarDays,
  CheckCircle2,
  Cloud,
  FileText,
  Globe,
  HardDrive,
  KeyRound,
  ListTodo,
  Loader2,
  Mail,
  Plug,
  RefreshCw,
  Save,
  Send,
  ShieldCheck,
  Inbox,
  Package,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { InfoRow } from "@/components/ui/info-row";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { SignalCard } from "@/components/settings/SignalCard";
import { SectionTabs } from "@/components/ui/section-tabs";
import { InboundSourcesCard } from "@/components/integrations/InboundSourcesCard";
import { OutboundSinksCard } from "@/components/integrations/OutboundSinksCard";
import { BrowserCard } from "@/components/integrations/BrowserCard";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { openUrl } from "@tauri-apps/plugin-opener";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import type { Delivery, GoogleConfig, GoogleStatus, SmtpSettings } from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";

const GOOGLE_SERVICES = [
  { icon: FileText, title: "Docs", description: "Markdown → documento nativo formatado" },
  { icon: HardDrive, title: "Drive", description: "Upload e organização por workspace" },
  { icon: CalendarDays, title: "Calendar", description: "Prazos e eventos a partir dos artefatos" },
  { icon: Mail, title: "Gmail", description: "Envio, rascunhos e leitura de threads" },
  { icon: ListTodo, title: "Tasks", description: "Backlog bidirecional (Keep só enterprise)" },
];

interface SmtpForm {
  host: string;
  port: number;
  username: string;
  fromEmail: string;
  useTls: boolean;
  password: string;
}

const EMPTY_FORM: SmtpForm = {
  host: "",
  port: 587,
  username: "",
  fromEmail: "",
  useTls: true,
  password: "",
};

export function IntegrationsPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const lastGoogleEvent = useAppStore((state) => state.lastGoogleEvent);
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  const [smtp, setSmtp] = useState<SmtpSettings | null>(null);
  const [form, setForm] = useState<SmtpForm>(EMPTY_FORM);
  const [deliveries, setDeliveries] = useState<Delivery[]>([]);
  const [testTo, setTestTo] = useState("");
  const [google, setGoogle] = useState<GoogleStatus | null>(null);
  const [googleConfig, setGoogleConfig] = useState<GoogleConfig | null>(null);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [googleBusy, setGoogleBusy] = useState(false);
  const [params, setParams] = useSearchParams();
  const [googleMessage, setGoogleMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [retrying, setRetrying] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const section = params.get("section") ?? "smtp";
  const tabs = [
    { id: "smtp", label: "SMTP", icon: Mail },
    { id: "google", label: "Google", icon: Cloud },
    { id: "signal", label: "Signal", icon: Plug },
    { id: "browser", label: "Browser", icon: Globe },
    { id: "inbound", label: "Entradas", icon: Inbox },
    { id: "sinks", label: "Saídas", icon: Package },
    { id: "deliveries", label: "Histórico", icon: Send },
    { id: "mcp", label: "MCP e segurança", icon: ShieldCheck },
  ] as const;
  const activeTab = tabs.some((tab) => tab.id === section) ? section : "smtp";
  function selectTab(id: string) {
    setParams({ section: id }, { replace: true });
  }

  const query = useMemo(
    () => (activeWorkspaceId ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}` : ""),
    [activeWorkspaceId],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) return;
    setLoading(true);
    try {
      const [settings, history, googleStatus, googleCfg] = await Promise.all([
        apiGet<SmtpSettings>(`/api/integrations/smtp${query}`),
        apiGet<Delivery[]>(`/api/integrations/deliveries${query}`),
        apiGet<GoogleStatus>(`/api/integrations/google/status${query}`).catch(() => null),
        apiGet<GoogleConfig>(`/api/integrations/google/config${query}`).catch(() => null),
      ]);
      setSmtp(settings);
      setDeliveries(history);
      setGoogle(googleStatus);
      setGoogleConfig(googleCfg);
      setClientId((current) => current || (googleCfg?.client_id_set ? "" : ""));
      setForm({
        host: settings.host ?? "",
        port: settings.port ?? 587,
        username: settings.username ?? "",
        fromEmail: settings.from_email ?? "",
        useTls: settings.use_tls,
        password: "",
      });
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    void load();
  }, [load, lastGoogleEvent?.workspace_id, lastGoogleEvent?.connected]);

  async function save() {
    if (!activeWorkspaceId) return;
    setSaving(true);
    setError(null);
    setMessage(null);
    try {
      await apiPut("/api/integrations/smtp", {
        workspace_id: activeWorkspaceId,
        host: form.host,
        port: form.port,
        username: form.username || null,
        from_email: form.fromEmail,
        use_tls: form.useTls,
        password: form.password || undefined,
      });
      setMessage("SMTP salvo.");
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function test() {
    if (!activeWorkspaceId) return;
    setTesting(true);
    setError(null);
    setMessage(null);
    try {
      await apiPost("/api/integrations/smtp/test", {
        workspace_id: activeWorkspaceId,
        to: testTo.trim() || null,
      });
      setMessage(
        testTo.trim()
          ? "Conexão ok; e-mail de teste enviado."
          : "Conexão SMTP ok.",
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setTesting(false);
    }
  }

  async function retryDelivery(id: string) {
    setRetrying(id);
    setError(null);
    setMessage(null);
    try {
      await apiPost<Delivery>(`/api/integrations/deliveries/${id}/retry`);
      await load();
      setMessage("Entrega reenviada.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setRetrying(null);
    }
  }

  async function saveGoogle() {
    setGoogleBusy(true);
    setError(null);
    setGoogleMessage(null);
    try {
      await apiPut("/api/integrations/google/config", {
        client_id: clientId.trim(),
        client_secret: clientSecret.trim() || undefined,
      });
      setGoogleMessage("Credenciais Google salvas.");
      setClientSecret("");
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setGoogleBusy(false);
    }
  }

  async function connectGoogle() {
    if (!activeWorkspaceId) return;
    setGoogleBusy(true);
    setError(null);
    setGoogleMessage(null);
    try {
      const response = await apiPost<{ auth_url: string }>(
        "/api/integrations/google/oauth/start",
        { workspace_id: activeWorkspaceId },
      );
      try {
        await openUrl(response.auth_url);
      } catch {
        window.open(response.auth_url, "_blank", "noopener,noreferrer");
      }
      setGoogleMessage(
        "Autorize no navegador. O callback volta para o OpenDesk; depois clique em Atualizar.",
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setGoogleBusy(false);
    }
  }

  async function disconnectGoogle() {
    if (!activeWorkspaceId) return;
    setGoogleBusy(true);
    try {
      await apiPost("/api/integrations/google/disconnect", {
        workspace_id: activeWorkspaceId,
      });
      setGoogleMessage("Conta Google desconectada.");
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setGoogleBusy(false);
    }
  }

  async function registerMcp() {
    if (!activeWorkspaceId) return;
    setGoogleBusy(true);
    try {
      await apiPost("/api/integrations/google/mcp/register", {
        workspace_id: activeWorkspaceId,
      });
      setGoogleMessage("MCP server opendesk-google registrado no opencode.jsonc.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setGoogleBusy(false);
    }
  }

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader eyebrow="credenciais por workspace" title="Integrações" />
        <EmptyState
          icon={Plug}
          title="Nenhum workspace ativo"
          description="As credenciais SMTP e Google são por workspace."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="credenciais por workspace"
        title="Integrações"
        description={`${workspace?.name ?? "Workspace"} · SMTP, Google Workspace e Signal. Segredos ficam criptografados em repouso (AES-256-GCM, 0600).`}
        actions={
          <Button variant="outline" size="sm" onClick={() => void load()}>
            <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
            Atualizar
          </Button>
        }
      />

      {message ? (
        <p className="flex items-center gap-2 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[11px] text-emerald-300">
          <CheckCircle2 size={13} />
          {message}
        </p>
      ) : null}
      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      <SectionTabs items={[...tabs]} value={activeTab} onChange={selectTab} />

      {activeTab === "smtp" ? (
        <Card id="integrations-smtp">
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <Mail size={15} className="text-zinc-500" />
                SMTP
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                Resumo e anexos dos runs concluídos.
              </p>
            </div>
            <Badge variant={smtp?.configured ? "green" : "default"}>
              {smtp?.configured ? "configurado" : "não configurado"}
            </Badge>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="grid grid-cols-[1fr_100px] gap-2">
              <label>
                <span className={labelClass}>Host</span>
                <input
                  value={form.host}
                  onChange={(event) => setForm({ ...form, host: event.target.value })}
                  placeholder="smtp.gmail.com"
                  className={inputClass}
                />
              </label>
              <label>
                <span className={labelClass}>Porta</span>
                <input
                  type="number"
                  value={form.port}
                  onChange={(event) =>
                    setForm({ ...form, port: Number(event.target.value) })
                  }
                  className={inputClass}
                />
              </label>
            </div>
            <div className="grid gap-2 md:grid-cols-2">
              <label>
                <span className={labelClass}>Usuário</span>
                <input
                  value={form.username}
                  onChange={(event) => setForm({ ...form, username: event.target.value })}
                  placeholder="usuario@gmail.com"
                  className={inputClass}
                />
              </label>
              <label>
                <span className={labelClass}>Remetente</span>
                <input
                  value={form.fromEmail}
                  onChange={(event) => setForm({ ...form, fromEmail: event.target.value })}
                  placeholder="OpenDesk <voce@empresa.com>"
                  className={inputClass}
                />
              </label>
            </div>
            <label>
              <span className={labelClass}>Senha / app password</span>
              <input
                type="password"
                value={form.password}
                onChange={(event) => setForm({ ...form, password: event.target.value })}
                placeholder={smtp?.password_set ? "•••••••• (deixe vazio para manter)" : "app password"}
                className={inputClass}
              />
            </label>
            <div className="flex items-center justify-between gap-3">
              <label className="flex items-center gap-2 text-[11px] text-zinc-400">
                <input
                  type="checkbox"
                  checked={form.useTls}
                  onChange={(event) => setForm({ ...form, useTls: event.target.checked })}
                  className="accent-blue-500"
                />
                usar TLS
              </label>
              <div className="flex gap-2">
                <Button variant="outline" size="sm" onClick={() => void test()} disabled={testing}>
                  {testing ? <Loader2 size={13} className="animate-spin" /> : <Send size={13} />}
                  Testar
                </Button>
                <Button size="sm" onClick={() => void save()} disabled={saving}>
                  {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
                  Salvar
                </Button>
              </div>
            </div>
            <div className="flex gap-2">
              <input
                value={testTo}
                onChange={(event) => setTestTo(event.target.value)}
                placeholder="e-mail para teste (opcional)"
                className={cn(inputClass, "h-8 text-[11px]")}
              />
            </div>
            <p className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[9px] leading-relaxed text-zinc-500">
              Gmail: use uma App Password. Porta 587 usa STARTTLS; porta 465 usa TLS
              implícito. O teste sem destinatário valida apenas a conexão.
            </p>
          </CardContent>
        </Card>
      ) : null}

      {activeTab === "google" ? (
        <Card id="integrations-google">
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <Cloud size={15} className="text-zinc-500" />
                Google Workspace
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                OAuth2 loopback + PKCE; Docs, Drive, Calendar, Gmail e Tasks.
              </p>
            </div>
            <Badge variant={google?.connected ? "green" : "default"}>
              {google?.connected ? "conectado" : "não conectado"}
            </Badge>
          </CardHeader>
          <CardContent className="space-y-3">
            <InfoRow label="Conta" value={google?.email ?? "não conectada"} />
            <InfoRow label="Callback" value={google?.callback_url ?? googleConfig?.callback_url ?? "—"} />
            <div className="grid grid-cols-[1fr_110px] gap-2">
              <label>
                <span className={labelClass}>Client ID</span>
                <input
                  value={clientId}
                  onChange={(event) => setClientId(event.target.value)}
                  placeholder={googleConfig?.client_id_set ? "já configurado (cole para trocar)" : "....apps.googleusercontent.com"}
                  className={inputClass}
                />
              </label>
              <label>
                <span className={labelClass}>Client secret</span>
                <input
                  type="password"
                  value={clientSecret}
                  onChange={(event) => setClientSecret(event.target.value)}
                  placeholder={googleConfig?.client_secret_set ? "••••••••" : "client secret"}
                  className={inputClass}
                />
              </label>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button size="sm" onClick={() => void saveGoogle()} disabled={googleBusy || !clientId.trim()}>
                {googleBusy ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
                Salvar credenciais
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => void connectGoogle()}
                disabled={googleBusy || !googleConfig?.client_id_set}
              >
                <KeyRound size={13} />
                {google?.connected ? "Reconectar" : "Conectar Google"}
              </Button>
              {google?.connected ? (
                <Button variant="destructive" size="sm" onClick={() => void disconnectGoogle()} disabled={googleBusy}>
                  Revogar
                </Button>
              ) : null}
              <Button variant="ghost" size="sm" onClick={() => void registerMcp()} disabled={googleBusy}>
                <Plug size={13} />
                Registrar MCP
              </Button>
            </div>
            {googleMessage ? (
              <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[10px] leading-relaxed text-emerald-300">
                {googleMessage}
              </p>
            ) : null}
            <p className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[9px] leading-relaxed text-zinc-500">
              No Google Cloud, crie um OAuth client do tipo <span className="font-mono">Desktop app</span>{" "}
              e adicione <span className="font-mono">{google?.callback_url ?? googleConfig?.callback_url ?? "http://127.0.0.1:53682/callback"}</span>{" "}
              como redirect URI autorizado. O escopo de Drive é <span className="font-mono">drive.file</span>.
            </p>
          </CardContent>
        </Card>
      ) : null}

      {activeTab === "signal" ? (
        <div id="integrations-signal">
          <SignalCard />
        </div>
      ) : null}

      {activeTab === "browser" ? <BrowserCard /> : null}

      {activeTab === "inbound" ? (
        <InboundSourcesCard workspaceId={activeWorkspaceId ?? ""} />
      ) : null}

      {activeTab === "sinks" ? (
        <OutboundSinksCard workspaceId={activeWorkspaceId ?? ""} />
      ) : null}

      {activeTab === "deliveries" ? (
      <Card id="integrations-deliveries">
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Send size={15} className="text-zinc-500" />
              Entregas recentes
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Idempotência por run + destinatário em integration_deliveries.
            </p>
          </div>
          <Badge variant="default">{deliveries.length}</Badge>
        </CardHeader>
        <CardContent>
          {deliveries.length === 0 ? (
            <EmptyState
              icon={Send}
              title="Nenhuma entrega ainda"
              description="Ao concluir um run com notify_email, o OpenDesk envia o resumo e os anexos e registra o status aqui."
            />
          ) : (
            <div className="space-y-2">
              {deliveries.map((delivery) => (
                <div
                  key={delivery.id}
                  className="flex flex-wrap items-center gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
                >
                  <Badge
                    variant={
                      delivery.status === "SUCCESS"
                        ? "green"
                        : delivery.status === "FAILED"
                          ? "red"
                          : "default"
                    }
                  >
                    {delivery.status}
                  </Badge>
                  <span className="font-mono text-[10px] text-zinc-300">{delivery.channel}</span>
                  <span className="min-w-0 flex-1 truncate font-mono text-[9px] text-zinc-500">
                    {delivery.idempotency_key}
                  </span>
                  <span className="font-mono text-[9px] text-zinc-500">
                    {delivery.attempts} tentativa(s) · {relativeTime(delivery.updated_at)}
                  </span>
                  {delivery.status === "FAILED" ? (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => void retryDelivery(delivery.id)}
                      disabled={retrying === delivery.id}
                      aria-label={`Reenviar entrega ${delivery.id}`}
                    >
                      {retrying === delivery.id ? (
                        <Loader2 size={12} className="animate-spin" />
                      ) : (
                        <RefreshCw size={12} />
                      )}
                      Reenviar
                    </Button>
                  ) : null}
                  {delivery.last_error ? (
                    <span className="w-full truncate text-[9px] text-red-300">
                      {delivery.last_error}
                    </span>
                  ) : null}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
      ) : null}

      {activeTab === "mcp" ? (
        <>
      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Plug size={15} className="text-zinc-500" />
              Serviços e ferramentas MCP
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              O mesmo executável expõe <span className="font-mono">opendesk mcp-google</span> via
              stdio e registra em <span className="font-mono">mcp.servers</span>.
            </p>
          </div>
          <Badge variant="purple">MCP bridge</Badge>
        </CardHeader>
        <CardContent>
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-5">
            {GOOGLE_SERVICES.map((service) => (
              <div
                key={service.title}
                className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3"
              >
                <service.icon size={15} className="text-zinc-500" />
                <p className="mt-2 text-[11px] font-medium text-zinc-300">{service.title}</p>
                <p className="mt-1 text-[9px] leading-relaxed text-zinc-500">
                  {service.description}
                </p>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="flex items-start gap-3 p-4">
          <ShieldCheck size={16} className="mt-0.5 text-emerald-400/70" />
          <p className="text-[11px] leading-relaxed text-zinc-500">
            Segredos são criptografados com AES-256-GCM e chave derivada da máquina,
            gravados em <span className="font-mono">~/.local/share/opendesk/secrets</span> com
            permissão 0600. Logs e eventos nunca incluem senhas.
          </p>
        </CardContent>
      </Card>
        </>
      ) : null}
    </PageBody>
  );
}
