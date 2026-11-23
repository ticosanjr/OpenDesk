import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  Boxes,
  CheckCircle2,
  Cloud,
  Globe,
  Inbox,
  Mail,
  Package,
  Plug,
  Radio,
  RefreshCw,
  Send,
  ShieldCheck,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { SignalCard } from "@/components/settings/SignalCard";
import { TelegramCard } from "@/components/settings/TelegramCard";
import { SectionNav } from "@/components/ui/section-nav";
import { InboundSourcesCard } from "@/components/integrations/InboundSourcesCard";
import { OutboundSinksCard } from "@/components/integrations/OutboundSinksCard";
import { BrowserCard } from "@/components/integrations/BrowserCard";
import { ChannelsCard } from "@/components/integrations/ChannelsCard";
import { McpHubCard } from "@/components/integrations/McpHubCard";
import { TicketInboxCard } from "@/components/integrations/TicketInboxCard";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { openExternalUrl } from "@/lib/runtime-env";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import type { Delivery, GoogleConfig, GoogleStatus, SmtpSettings } from "@/lib/types";
import { EMPTY_FORM, type SmtpForm } from "./integrations/smtp-form";
import { SmtpCard } from "./integrations/SmtpCard";
import { GoogleWorkspaceCard } from "./integrations/GoogleWorkspaceCard";
import { DeliveriesCard } from "./integrations/DeliveriesCard";
import { McpServicesSection } from "./integrations/McpServicesSection";

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
    { id: "smtp", label: "SMTP", icon: Mail, group: "Mensageria" },
    { id: "google", label: "Google", icon: Cloud, group: "Mensageria" },
    { id: "signal", label: "Signal", icon: Plug, group: "Mensageria" },
    { id: "telegram", label: "Telegram", icon: Send, group: "Mensageria" },
    { id: "channels", label: "Canais e tickets", icon: Radio, group: "Mensageria" },
    { id: "browser", label: "Browser", icon: Globe, group: "I/O" },
    { id: "inbound", label: "Entradas", icon: Inbox, group: "I/O" },
    { id: "sinks", label: "Saídas", icon: Package, group: "I/O" },
    { id: "deliveries", label: "Histórico", icon: Send, group: "Sistema" },
    { id: "hub", label: "MCP Hub", icon: Boxes, group: "Sistema" },
    { id: "mcp", label: "MCP e segurança", icon: ShieldCheck, group: "Sistema" },
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
      await openExternalUrl(response.auth_url);
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
        <PageHeader title="Integrações" />
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
        title="Integrações"
        description={`${workspace?.name ?? "Workspace"} · SMTP, Google Workspace, Signal e Telegram. Segredos ficam criptografados em repouso (AES-256-GCM, 0600).`}
        actions={
          <Button variant="outline" size="sm" onClick={() => void load()}>
            <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
            Atualizar
          </Button>
        }
      />

      {message ? (
        <p className="flex items-center gap-2 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[13px] text-emerald-300">
          <CheckCircle2 size={13} />
          {message}
        </p>
      ) : null}
      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="grid gap-4 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-start">
        <SectionNav
          items={tabs}
          value={activeTab}
          onChange={selectTab}
          ariaLabel="Seções de integrações"
        />
        <div className="min-w-0 space-y-4">

      {activeTab === "smtp" ? (
        <SmtpCard
          smtp={smtp}
          form={form}
          setForm={setForm}
          testTo={testTo}
          setTestTo={setTestTo}
          testing={testing}
          saving={saving}
          onTest={() => void test()}
          onSave={() => void save()}
        />
      ) : null}

      {activeTab === "google" ? (
        <GoogleWorkspaceCard
          google={google}
          googleConfig={googleConfig}
          clientId={clientId}
          onClientIdChange={setClientId}
          clientSecret={clientSecret}
          onClientSecretChange={setClientSecret}
          busy={googleBusy}
          message={googleMessage}
          onSave={() => void saveGoogle()}
          onConnect={() => void connectGoogle()}
          onDisconnect={() => void disconnectGoogle()}
          onRegisterMcp={() => void registerMcp()}
        />
      ) : null}

      {activeTab === "signal" ? (
        <div id="integrations-signal">
          <SignalCard />
        </div>
      ) : null}

      {activeTab === "telegram" ? (
        <div id="integrations-telegram">
          <TelegramCard />
        </div>
      ) : null}

      {activeTab === "channels" && activeWorkspaceId ? (
        <div className="space-y-4">
          <ChannelsCard workspaceId={activeWorkspaceId} />
          <TicketInboxCard workspaceId={activeWorkspaceId} />
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
        <DeliveriesCard
          deliveries={deliveries}
          retrying={retrying}
          onRetry={(id) => void retryDelivery(id)}
        />
      ) : null}

      {activeTab === "hub" && activeWorkspaceId ? (
        <McpHubCard workspaceId={activeWorkspaceId} />
      ) : null}
      {activeTab === "mcp" ? (
        <McpServicesSection />
      ) : null}
        </div>
      </div>
    </PageBody>
  );
}
