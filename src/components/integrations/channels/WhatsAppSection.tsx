import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { inputClass, type WhatsAppConfig } from "./types";

export function WhatsAppSection({
  whatsapp,
  onChange,
  tokenSet,
  accessTokenSet,
  appSecretSet,
  accessToken,
  onAccessTokenChange,
  appSecret,
  onAppSecretChange,
  sendTo,
  onSendToChange,
  sendText,
  onSendTextChange,
  busy,
  onSave,
  onSend,
}: {
  whatsapp: WhatsAppConfig;
  onChange: (next: WhatsAppConfig) => void;
  tokenSet: boolean;
  accessTokenSet: boolean;
  appSecretSet: boolean;
  accessToken: string;
  onAccessTokenChange: (value: string) => void;
  appSecret: string;
  onAppSecretChange: (value: string) => void;
  sendTo: string;
  onSendToChange: (value: string) => void;
  sendText: string;
  onSendTextChange: (value: string) => void;
  busy: string | null;
  onSave: () => void;
  onSend: () => void;
}) {
  return (
    <div className="grid gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5 sm:grid-cols-2">
      <label className="flex items-center gap-2 text-[13px] text-zinc-300">
        <input
          type="checkbox"
          checked={whatsapp.enabled}
          onChange={(event) => onChange({ ...whatsapp, enabled: event.target.checked })}
          className="h-4 w-4 accent-blue-500"
        />
        WhatsApp (estrutura)
        {tokenSet ? <Badge variant="green">webhook</Badge> : null}
      </label>
      <select
        value={whatsapp.provider}
        onChange={(event) => onChange({ ...whatsapp, provider: event.target.value })}
        className={inputClass}
      >
        <option value="structure">structure (sem conector)</option>
        <option value="cloud-api">cloud-api (planejado)</option>
        <option value="baileys">baileys (planejado)</option>
      </select>
      <input
        value={whatsapp.phone_number}
        onChange={(event) => onChange({ ...whatsapp, phone_number: event.target.value })}
        placeholder="+55 11 99999-0000"
        className={inputClass}
      />
      <input
        value={whatsapp.phone_number_id}
        onChange={(event) => onChange({ ...whatsapp, phone_number_id: event.target.value })}
        placeholder="phone_number_id (Meta)"
        className={inputClass}
      />
      <input
        value={whatsapp.business_account_id}
        onChange={(event) =>
          onChange({ ...whatsapp, business_account_id: event.target.value })
        }
        placeholder="business account id (opcional)"
        className={inputClass}
      />
      <input
        value={whatsapp.api_version}
        onChange={(event) => onChange({ ...whatsapp, api_version: event.target.value })}
        placeholder="v21.0"
        className={inputClass}
      />
      <input
        value={whatsapp.notes}
        onChange={(event) => onChange({ ...whatsapp, notes: event.target.value })}
        placeholder="observações do número/conta"
        className={inputClass}
      />
      <input
        type="password"
        value={accessToken}
        onChange={(event) => onAccessTokenChange(event.target.value)}
        placeholder={accessTokenSet ? "token salvo (deixe vazio para manter)" : "access token (Meta)"}
        className={inputClass}
        autoComplete="new-password"
      />
      <input
        type="password"
        value={appSecret}
        onChange={(event) => onAppSecretChange(event.target.value)}
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
            onClick={onSave}
          >
            Salvar
          </Button>
        </div>
      </div>
      <div className="sm:col-span-2 flex flex-wrap items-end gap-2 border-t border-zinc-800/60 pt-2">
        <input
          value={sendTo}
          onChange={(event) => onSendToChange(event.target.value)}
          placeholder="destinatário (5511999...)"
          className={`${inputClass} w-44 font-mono`}
        />
        <input
          value={sendText}
          onChange={(event) => onSendTextChange(event.target.value)}
          placeholder="texto de teste"
          className={`${inputClass} flex-1`}
        />
        <Button
          variant="outline"
          size="sm"
          className="h-8"
          disabled={busy === "send" || !accessTokenSet || !whatsapp.phone_number_id}
          onClick={onSend}
        >
          Enviar teste
        </Button>
      </div>
    </div>
  );
}
