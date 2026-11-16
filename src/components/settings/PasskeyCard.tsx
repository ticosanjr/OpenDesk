import { useCallback, useEffect, useState } from "react";
import { Fingerprint, KeyRound, Loader2, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { relativeTime } from "@/lib/format";
import {
  deletePasskey,
  fetchPasskeyStatus,
  hasSecureOrigin,
  listPasskeys,
  passkeySupported,
  registerPasskey,
  type PasskeyRecord,
  type PasskeyStatus,
} from "@/lib/passkey";
import { toast } from "@/store/toast-store";

const inputClass =
  "h-8 w-full rounded border border-zinc-700 bg-zinc-950 px-2 text-[12.5px] text-zinc-200";

export function PasskeyCard() {
  const [status, setStatus] = useState<PasskeyStatus | null>(null);
  const [records, setRecords] = useState<PasskeyRecord[]>([]);
  const [label, setLabel] = useState("este dispositivo");
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [nextStatus, list] = await Promise.all([
        fetchPasskeyStatus(),
        listPasskeys(),
      ]);
      setStatus(nextStatus);
      setRecords(list);
    } catch {
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function register() {
    setBusy("register");
    try {
      await registerPasskey(label.trim() || "este dispositivo");
      await load();
      toast({
        tone: "success",
        title: "Passkey cadastrada",
        description: "Você já pode entrar sem digitar o token neste dispositivo.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao cadastrar a passkey",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function remove(id: string) {
    setBusy(id);
    try {
      await deletePasskey(id);
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao remover",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  const supported = passkeySupported();
  const secure = hasSecureOrigin();

  return (
    <Card id="settings-passkeys" className="scroll-mt-14">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Fingerprint size={15} className="text-zinc-500" />
            Passkeys (WebAuthn)
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Login sem token no PWA. A cerimônia é validada no servidor e a sessão
            emitida continua revogável em Configurações → Sessões remotas
            (API). Passkeys exigem HTTPS ou localhost.
          </p>
        </div>
        <Badge variant={status?.enabled ? "green" : "default"}>
          {status?.registered ?? 0} cadastrada(s)
        </Badge>
      </CardHeader>
      <CardContent className="space-y-3">
        {!supported ? (
          <p className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-[12px] text-amber-200">
            Este navegador não expõe WebAuthn.
          </p>
        ) : !secure ? (
          <p className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-[12px] text-amber-200">
            A origem atual não é segura para passkeys. Use HTTPS ou localhost.
          </p>
        ) : null}
        {status?.origin_error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            {status.origin_error}
          </p>
        ) : null}
        {status?.origin ? (
          <p className="font-mono text-[11px] text-zinc-600">
            relying party: {status.origin}
          </p>
        ) : null}

        <div className="flex flex-wrap items-end gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
          <label className="min-w-40 flex-1">
            <span className="mb-1 block text-[11.5px] uppercase tracking-[0.1em] text-zinc-500">
              Rótulo
            </span>
            <input
              value={label}
              onChange={(event) => setLabel(event.target.value)}
              className={inputClass}
            />
          </label>
          <Button
            size="sm"
            className="h-8"
            disabled={busy === "register" || !supported || !secure}
            onClick={() => void register()}
          >
            {busy === "register" ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <KeyRound size={12} />
            )}
            Cadastrar passkey
          </Button>
        </div>

        {records.length === 0 ? (
          <EmptyState
            title="Nenhuma passkey"
            description="Cadastre uma para entrar no PWA sem colar o token."
          />
        ) : (
          <div className="space-y-1.5">
            {records.map((record) => (
              <div
                key={record.id}
                className="flex items-center justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
              >
                <div className="min-w-0">
                  <p className="text-[13px] text-zinc-200">{record.label}</p>
                  <p className="truncate font-mono text-[10.5px] text-zinc-600">
                    {record.credential_id.slice(0, 24)}… · criada{" "}
                    {relativeTime(record.created_at)}
                    {record.last_used_at
                      ? ` · usada ${relativeTime(record.last_used_at)}`
                      : ""}
                  </p>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2"
                  disabled={busy === record.id}
                  onClick={() => void remove(record.id)}
                >
                  <Trash2 size={11} />
                </Button>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
