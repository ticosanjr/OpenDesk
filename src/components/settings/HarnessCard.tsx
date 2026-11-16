import { useCallback, useEffect, useState } from "react";
import { Check, Loader2, RefreshCw, Workflow } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPatch, apiPost } from "@/lib/api";
import { toast } from "@/store/toast-store";

interface HarnessView {
  id: string;
  name: string;
  kind: string;
  command?: string | null;
  args_json: string;
  health_args_json: string;
  status: string;
  is_default: boolean;
}

function statusTone(status: string): "green" | "red" | "yellow" | "default" {
  switch (status) {
    case "READY":
      return "green";
    case "MISSING":
      return "red";
    case "ERROR":
      return "yellow";
    default:
      return "default";
  }
}

export function HarnessCard() {
  const [harnesses, setHarnesses] = useState<HarnessView[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<Record<string, { command: string; args: string }>>(
    {},
  );

  const load = useCallback(async () => {
    try {
      const list = await apiGet<HarnessView[]>("/api/adapters");
      setHarnesses(list);
      setDrafts(
        Object.fromEntries(
          list.map((item) => [
            item.id,
            {
              command: item.command ?? "",
              args: (JSON.parse(item.args_json) as string[]).join(" "),
            },
          ]),
        ),
      );
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar os harnesses",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function makeDefault(id: string) {
    setBusy(id);
    try {
      await apiPost("/api/adapters/default", { harness: id });
      await load();
      toast({ tone: "success", title: "Harness padrão atualizado" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao definir o padrão",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function save(item: HarnessView) {
    const draft = drafts[item.id];
    if (!draft) return;
    setBusy(item.id);
    try {
      await apiPatch(`/api/adapters/${item.id}`, {
        command: draft.command.trim() || null,
        args: draft.args.trim() ? draft.args.trim().split(/\s+/) : [],
      });
      await load();
      toast({ tone: "success", title: `${item.name} atualizado` });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function check(item: HarnessView) {
    setBusy(item.id);
    try {
      const health = await apiPost<{ status: string; version?: string; error?: string }>(
        `/api/adapters/${item.id}/health`,
      );
      toast({
        tone: health.status === "READY" ? "success" : "error",
        title:
          health.status === "READY"
            ? `${item.name} pronto`
            : `${item.name} indisponível`,
        description: health.version ?? health.error,
      });
      await load();
    } finally {
      setBusy(null);
    }
  }

  if (loading) {
    return (
      <Card id="settings-harness" className="scroll-mt-14">
        <CardContent className="flex items-center gap-2 py-6 text-[13px] text-zinc-500">
          <Loader2 size={14} className="animate-spin" />
          Carregando harnesses…
        </CardContent>
      </Card>
    );
  }

  return (
    <Card id="settings-harness" className="scroll-mt-14">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Workflow size={15} className="text-zinc-500" />
            Harnesses de execução
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            O OpenDesk orquestra; o harness executa. O padrão vale para tarefas e
            rotinas sem escolha explícita. Comandos CLI usam os placeholders{" "}
            <code className="font-mono text-[11px]">{"{prompt}"}</code> e{" "}
            <code className="font-mono text-[11px]">{"{model}"}</code>.
          </p>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {harnesses.map((item) => {
          const draft = drafts[item.id] ?? { command: "", args: "" };
          return (
            <div
              key={item.id}
              className="space-y-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <span className="text-[13.5px] font-medium text-zinc-200">
                    {item.name}
                  </span>
                  <span className="font-mono text-[11.5px] text-zinc-500">
                    {item.id}
                  </span>
                  <Badge variant={statusTone(item.status)}>{item.status}</Badge>
                  {item.is_default ? (
                    <Badge variant="blue">
                      <Check size={10} />
                      padrão
                    </Badge>
                  ) : null}
                </div>
                <div className="flex items-center gap-1.5">
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7"
                    disabled={busy === item.id}
                    onClick={() => void check(item)}
                  >
                    <RefreshCw size={11} />
                    Verificar
                  </Button>
                  {!item.is_default ? (
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7"
                      disabled={busy === item.id}
                      onClick={() => void makeDefault(item.id)}
                    >
                      Tornar padrão
                    </Button>
                  ) : null}
                </div>
              </div>
              {item.kind === "cli" ? (
                <div className="grid gap-2 sm:grid-cols-[180px_minmax(0,1fr)_auto]">
                  <input
                    value={draft.command}
                    onChange={(event) =>
                      setDrafts((current) => ({
                        ...current,
                        [item.id]: { ...draft, command: event.target.value },
                      }))
                    }
                    placeholder="comando"
                    className="h-8 rounded border border-zinc-700 bg-zinc-950 px-2 font-mono text-[12px] text-zinc-200"
                  />
                  <input
                    value={draft.args}
                    onChange={(event) =>
                      setDrafts((current) => ({
                        ...current,
                        [item.id]: { ...draft, args: event.target.value },
                      }))
                    }
                    placeholder="argumentos separados por espaço"
                    className="h-8 rounded border border-zinc-700 bg-zinc-950 px-2 font-mono text-[12px] text-zinc-200"
                  />
                  <Button
                    size="sm"
                    className="h-8"
                    disabled={busy === item.id}
                    onClick={() => void save(item)}
                  >
                    Salvar
                  </Button>
                </div>
              ) : (
                <p className="text-[12px] text-zinc-500">
                  Sessões e permissões vêm do daemon OpenCode local.
                </p>
              )}
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}
