import { useState, type ReactNode } from "react";
import { KeyRound, ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

const TOKEN_KEY = "opendesk_api_token";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function hasEnvToken(): boolean {
  return Boolean(import.meta.env.VITE_OPENDESK_TOKEN);
}

export function RemoteAuthGate({ children }: { children: ReactNode }) {
  const [token, setToken] = useState(() => {
    try {
      return window.localStorage.getItem(TOKEN_KEY) ?? "";
    } catch {
      return "";
    }
  });
  const [error, setError] = useState<string | null>(null);

  if (isTauri() || hasEnvToken()) return <>{children}</>;
  if (token) return <>{children}</>;

  function signIn() {
    const value = token.trim();
    if (!value) {
      setError("Informe o token da API local.");
      return;
    }
    try {
      window.localStorage.setItem(TOKEN_KEY, value);
      window.location.reload();
    } catch {
      setError("O navegador bloqueou o armazenamento local.");
    }
  }

  return (
    <div className="flex min-h-dvh items-center justify-center bg-zinc-950 p-4 text-zinc-200">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-[15px]">
            <ShieldCheck size={16} className="text-emerald-300" />
            Acesso remoto ao OpenDesk
          </CardTitle>
          <p className="mt-1 text-[12.5px] leading-relaxed text-zinc-500">
            Este PWA está conectado ao opendeskd. Informe o token da API local
            para abrir o control plane neste dispositivo.
          </p>
        </CardHeader>
        <CardContent className="space-y-3">
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
                  if (event.key === "Enter") signIn();
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
            <Button size="sm" onClick={signIn}>
              Entrar
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
