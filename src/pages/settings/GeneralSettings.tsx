import {
  Check,
  ExternalLink,
  FolderOpen,
  HardDrive,
  Loader2,
  Save,
  Settings,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { FilePreferencesCard } from "@/components/settings/FilePreferencesCard";
import { cn } from "@/lib/utils";

export function GeneralSettings({
  root,
  loading,
  saving,
  saved,
  error,
  onRootChange,
  onChooseRoot,
  onSaveRoot,
  onReveal,
  onSelectTab,
}: {
  root: string;
  loading: boolean;
  saving: boolean;
  saved: boolean;
  error: string | null;
  onRootChange: (value: string) => void;
  onChooseRoot: () => void;
  onSaveRoot: () => void;
  onReveal: () => void;
  onSelectTab: (id: string) => void;
}) {
  return (
    <div id="settings-general" className="grid scroll-mt-14 gap-3 lg:grid-cols-2">
      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <HardDrive size={15} className="text-zinc-500" />
              Pasta padrão de workspaces
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Novos workspaces são criados como <span className="font-mono">{"{raiz}/{slug-do-nome}"}</span>.
            </p>
          </div>
          {saved ? (
            <Badge variant="green">
              <Check size={10} />
              salvo
            </Badge>
          ) : null}
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex gap-2">
            <div className="relative min-w-0 flex-1">
              <FolderOpen
                size={15}
                className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500"
              />
              <input
                value={root}
                onChange={(event) => onRootChange(event.target.value)}
                placeholder="~/Documents/OpenDesk Workareas"
                className={cn(
                  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 pl-9 pr-3 font-mono text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60",
                  loading && "opacity-60",
                )}
              />
            </div>
            <Button variant="outline" size="sm" className="h-9" onClick={() => onChooseRoot()}>
              Escolher
            </Button>
            <Button size="sm" className="h-9" onClick={() => onSaveRoot()} disabled={saving || loading}>
              {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
              Salvar
            </Button>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onReveal()}
              disabled={!root}
            >
              <ExternalLink size={12} />
              Abrir pasta
            </Button>
            <p className="text-[11.5px] text-zinc-500">
              A pasta é criada automaticamente se não existir.
            </p>
          </div>
          {error ? (
            <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
              {error}
            </p>
          ) : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Settings size={15} className="text-zinc-500" />
              Segundo plano
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              O OpenDesk continua executando fila, cron e notificações com a janela
              escondida.
            </p>
          </div>
          <Badge variant="green">bandeja</Badge>
        </CardHeader>
        <CardContent className="space-y-2 text-[13px] leading-relaxed text-zinc-500">
          <p>
            Fechar pelo “X” mantém o app na bandeja por padrão. Para encerrar de
            verdade, use <span className="text-zinc-300">Sair</span> no menu do
            ícone da bandeja.
          </p>
          <p>
            O comportamento do “X” pode ser alterado em{" "}
            <button
              type="button"
              onClick={() => onSelectTab("notifications")}
              className="text-blue-300 underline-offset-2 hover:underline"
            >
              Notificações
            </button>
            .
          </p>
        </CardContent>
      </Card>
      <FilePreferencesCard />
    </div>
  );
}
