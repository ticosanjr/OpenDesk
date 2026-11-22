import { Loader2, Mail, Save, Send } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import type { SmtpSettings } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import type { SmtpForm } from "./smtp-form";

export function SmtpCard({
  smtp,
  form,
  setForm,
  testTo,
  setTestTo,
  testing,
  saving,
  onTest,
  onSave,
}: {
  smtp: SmtpSettings | null;
  form: SmtpForm;
  setForm: (next: SmtpForm) => void;
  testTo: string;
  setTestTo: (value: string) => void;
  testing: boolean;
  saving: boolean;
  onTest: () => void;
  onSave: () => void;
}) {
  return (
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
          <label className="flex items-center gap-2 text-[13.5px] text-zinc-400">
            <Switch
              checked={form.useTls}
              onCheckedChange={(checked) => setForm({ ...form, useTls: checked })}
              label="Usar TLS"
            />
            usar TLS
          </label>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={onTest} disabled={testing}>
              {testing ? <Loader2 size={13} className="animate-spin" /> : <Send size={13} />}
              Testar
            </Button>
            <Button size="sm" onClick={onSave} disabled={saving}>
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
            className={cn(inputClass, "h-8 text-[13px]")}
          />
        </div>
        <p className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[11.5px] leading-relaxed text-zinc-500">
          Gmail: use uma App Password. Porta 587 usa STARTTLS; porta 465 usa TLS
          implícito. O teste sem destinatário valida apenas a conexão.
        </p>
      </CardContent>
    </Card>
  );
}
