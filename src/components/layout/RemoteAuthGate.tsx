import { useEffect, useState, type ReactNode } from "react";
import { Fingerprint, KeyRound, Loader2, ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  fetchPasskeyStatus,
  hasSecureOrigin,
  loginWithPasskey,
  passkeySupported,
  type PasskeyStatus,
} from "@/lib/passkey";

const TOKEN_KEY = "opendesk_api_token";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function hasEnvToken(): boolean {
  return Boolean(import.meta.env.VITE_OPENDESK_TOKEN);
}

function readStoredToken(): string {
  try {
    return window.localStorage.getItem(TOKEN_KEY) ?? "";
  } catch {
    return "";
  }
}

export function RemoteAuthGate({ children }: { children: ReactNode }) {
  const [storedToken] = useState(readStoredToken);
  const [token, setToken] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [passkey, setPasskey] = useState<PasskeyStatus | null>(null);
  const [passkeyBusy, setPasskeyBusy] = useState(false);

  useEffect(() => {
    if (isTauri() || hasEnvToken() || storedToken) return;
    void fetchPasskeyStatus()
      .then(setPasskey)
      .catch(() => setPasskey(null));
  }, [storedToken]);

  if (isTauri() || hasEnvToken()) return <>{children}</>;
  if (storedToken) return <>{children}</>;

  async function signIn() {
    const value = token.trim();
    if (!value) {
      setError("Informe o token da API local.");
      return;
    }
    let stored = value;
    try {
      const response = await fetch("/api/auth/session", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${value}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ label: "pwa", ttl_days: 30 }),
      });
      if (response.ok) {
        const data = (await response.json()) as { token?: string };
        if (data.token) stored = data.token;
      }
    } catch {
      // Older daemons may not expose /api/auth/session; keep the raw token.
    }
    try {
      window.localStorage.setItem(TOKEN_KEY, stored);
      window.location.reload();
    } catch {
      setError("O navegador bloqueou o armazenamento local.");
    }
  }

  async function passkeySignIn() {
    setPasskeyBusy(true);
    setError(null);
    try {
      await loginWithPasskey();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setPasskeyBusy(false);
    }
  }

  const canUsePasskey =
    Boolean(passkey?.enabled) && passkeySupported() && hasSecureOrigin();

  return (
    <div className="flex min-h-dvh items-center justify-center bg-zinc-950 p-4 text-zinc-200">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-[15px]">
            <ShieldCheck size={16} className="text-emerald-300" />
            Acesso remoto ao OpenDesk
          </CardTitle>
          <p className="mt-1 text-[12.5px] leading-relaxed text-zinc-500">
            Este PWA está conectado ao opendeskd. Entre com uma passkey ou com o
            token da API local.
          </p>
        </CardHeader>
        <CardContent className="space-y-3">
          {canUsePasskey ? (
            <Button
              className="w-full"
              onClick={() => void passkeySignIn()}
              disabled={passkeyBusy}
            >
              {passkeyBusy ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <Fingerprint size={14} />
              )}
              Entrar com passkey
            </Button>
          ) : null}

          {passkey?.enabled && !hasSecureOrigin() ? (
            <p className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-[11.5px] text-amber-200">
              Passkeys exigem HTTPS ou localhost. Abra o PWA por um domínio com
              TLS para usá-las.
            </p>
          ) : null}

          <label>
            <span className="mb-1 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
              Token da API
            </span>
            <div className="flex items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/60 px-3">
              <KeyRound size={14} className="text-zinc-500" />
              <input
                type="password"
                value={token}
                onChange={(event) => {
                  setToken(event.target.value);
                  setError(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void signIn();
                }}
                placeholder="cole o token de api.json"
                className="h-10 min-w-0 flex-1 bg-transparent font-mono text-[12.5px] text-zinc-200 outline-none placeholder:text-zinc-600"
                autoComplete="current-password"
              />
            </div>
          </label>
          {error ? (
            <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
              {error}
            </p>
          ) : null}
          <div className="flex items-center justify-between gap-2">
            <p className="text-[11.5px] text-zinc-500">
              O token fica somente neste navegador.
            </p>
            <Button size="sm" onClick={() => void signIn()}>
              Entrar
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
