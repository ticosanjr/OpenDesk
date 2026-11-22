import { Cloud, KeyRound, Loader2, Plug, Save } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { InfoRow } from "@/components/ui/info-row";
import type { GoogleConfig, GoogleStatus } from "@/lib/types";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";

export function GoogleWorkspaceCard({
  google,
  googleConfig,
  clientId,
  onClientIdChange,
  clientSecret,
  onClientSecretChange,
  busy,
  message,
  onSave,
  onConnect,
  onDisconnect,
  onRegisterMcp,
}: {
  google: GoogleStatus | null;
  googleConfig: GoogleConfig | null;
  clientId: string;
  onClientIdChange: (value: string) => void;
  clientSecret: string;
  onClientSecretChange: (value: string) => void;
  busy: boolean;
  message: string | null;
  onSave: () => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onRegisterMcp: () => void;
}) {
  return (
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
              onChange={(event) => onClientIdChange(event.target.value)}
              placeholder={googleConfig?.client_id_set ? "já configurado (cole para trocar)" : "....apps.googleusercontent.com"}
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Client secret</span>
            <input
              type="password"
              value={clientSecret}
              onChange={(event) => onClientSecretChange(event.target.value)}
              placeholder={googleConfig?.client_secret_set ? "••••••••" : "client secret"}
              className={inputClass}
            />
          </label>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button size="sm" onClick={onSave} disabled={busy || !clientId.trim()}>
            {busy ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
            Salvar credenciais
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={onConnect}
            disabled={busy || !googleConfig?.client_id_set}
          >
            <KeyRound size={13} />
            {google?.connected ? "Reconectar" : "Conectar Google"}
          </Button>
          {google?.connected ? (
            <Button variant="destructive" size="sm" onClick={onDisconnect} disabled={busy}>
              Revogar
            </Button>
          ) : null}
          <Button variant="ghost" size="sm" onClick={onRegisterMcp} disabled={busy}>
            <Plug size={13} />
            Registrar MCP
          </Button>
        </div>
        {message ? (
          <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[12px] leading-relaxed text-emerald-300">
            {message}
          </p>
        ) : null}
        <p className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[11.5px] leading-relaxed text-zinc-500">
          No Google Cloud, crie um OAuth client do tipo <span className="font-mono">Desktop app</span>{" "}
          e adicione <span className="font-mono">{google?.callback_url ?? googleConfig?.callback_url ?? "http://127.0.0.1:53682/callback"}</span>{" "}
          como redirect URI autorizado. O escopo de Drive é <span className="font-mono">drive.file</span>.
        </p>
      </CardContent>
    </Card>
  );
}
