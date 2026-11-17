import { useCallback, useEffect, useState } from "react";
import {
  ArrowLeft,
  CheckCircle2,
  Loader2,
  RefreshCw,
  RotateCcw,
  XCircle,
} from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiGet, apiPost } from "@/lib/api";
import { recipeRunStatusLabel, recipeRunStatusTone } from "@/lib/status";
import { relativeTime } from "@/lib/format";
import { toast } from "@/store/toast-store";

interface RecipeRunDetail {
  id: string;
  recipe_id: string;
  status: string;
  current_step_id?: string | null;
  trigger_context_json: string;
  step_outputs_json: string;
  output_dir_path: string;
  dry_run: boolean;
  error_message?: string | null;
  started_at: string;
  completed_at?: string | null;
  cancel_requested?: boolean;
}

interface RecipeRunNode {
  id: string;
  node_index: number;
  node_id: string;
  node_path?: string | null;
  parent_path?: string | null;
  item_key?: string | null;
  kind: string;
  status: string;
  attempts: number;
  output_json?: string | null;
  error_message?: string | null;
  started_at?: string | null;
  finished_at?: string | null;
}

interface HitlRequest {
  token: string;
  title?: string | null;
  question_markdown?: string | null;
  context_json: string;
  options_json: string;
  interaction_type: string;
  status: string;
  expires_at: string;
}

function parseJson(value: string): unknown {
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

function statusVariant(status: string) {
  if (status === "DONE") return "green" as const;
  if (status === "FAILED") return "red" as const;
  if (status === "WAITING_HITL") return "yellow" as const;
  if (status === "RUNNING") return "blue" as const;
  return "default" as const;
}

function RecipeNodeTree({
  node,
  children,
  depth,
}: {
  node: RecipeRunNode;
  children: Map<string, RecipeRunNode[]>;
  depth: number;
}) {
  const key = node.node_path ?? node.node_id;
  const nested = children.get(key) ?? [];
  return (
    <div className={depth > 0 ? "border-l border-zinc-800/70 pl-3" : undefined}>
      <div className="flex flex-wrap items-start justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-1.5">
            <Badge variant={statusVariant(node.status)}>{node.status}</Badge>
            <span className="font-mono text-[11px] text-zinc-500">#{node.node_index}</span>
            <span className="text-[13px] text-zinc-200">
              {node.node_path ?? node.node_id}
            </span>
            <Badge variant="default">{node.kind}</Badge>
            {node.item_key ? (
              <span className="font-mono text-[11px] text-zinc-500">
                item {node.item_key}
              </span>
            ) : null}
            {node.attempts > 1 ? (
              <span className="text-[11px] text-zinc-500">{node.attempts} tentativas</span>
            ) : null}
          </div>
          {node.error_message ? (
            <p className="mt-1 whitespace-pre-wrap text-[11.5px] text-red-300">
              {node.error_message}
            </p>
          ) : null}
          {node.output_json ? (
            <pre className="mt-1 max-h-24 overflow-auto whitespace-pre-wrap font-mono text-[10.5px] text-zinc-500">
              {node.output_json}
            </pre>
          ) : null}
        </div>
        <span className="text-[11px] text-zinc-600">
          {node.started_at ? relativeTime(node.started_at) : "não iniciado"}
          {node.finished_at ? ` · fim ${relativeTime(node.finished_at)}` : ""}
        </span>
      </div>
      {nested.length > 0 ? (
        <div className="mt-1.5 space-y-1.5">
          {nested.map((child) => (
            <RecipeNodeTree
              key={child.id}
              node={child}
              children={children}
              depth={depth + 1}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

export function RecipeRunDetailPage() {
  const { runId } = useParams<{ runId: string }>();
  const [data, setData] = useState<{
    run: RecipeRunDetail;
    hitl: HitlRequest[];
    nodes: RecipeRunNode[];
  } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const navigate = useNavigate();

  const load = useCallback(async () => {
    if (!runId) return;
    setLoading(true);
    try {
      const value = await apiGet<{
        run: RecipeRunDetail;
        hitl: HitlRequest[];
        nodes: RecipeRunNode[];
      }>(`/api/recipes/runs/${encodeURIComponent(runId)}`);
      setData({
        run: value.run,
        hitl: Array.isArray(value.hitl) ? value.hitl : [],
        nodes: Array.isArray(value.nodes) ? value.nodes : [],
      });
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [runId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function resolveHitl(
    request: HitlRequest,
    resolution: "APPROVED" | "REJECTED" | "CHOICE",
    optionIndex?: number,
    text?: string,
  ) {
    setBusy(request.token);
    try {
      await apiPost(`/api/recipes/hitl/${encodeURIComponent(request.token)}/resolve`, {
        resolution,
        option_index: optionIndex,
        text,
      });
      toast({
        tone: "success",
        title:
          resolution === "APPROVED"
            ? "Checkpoint aprovado"
            : resolution === "REJECTED"
              ? "Checkpoint rejeitado"
              : "Opção registrada",
      });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível responder",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function cancelRun() {
    if (!runId) return;
    setBusy("cancel");
    try {
      await apiPost(`/api/recipes/runs/${encodeURIComponent(runId)}/cancel`);
      toast({
        tone: "info",
        title: "Cancelamento solicitado",
        description: "A receita vai parar no próximo limite seguro.",
      });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível cancelar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function retryRun() {
    if (!runId) return;
    setBusy("retry");
    try {
      await apiPost(`/api/recipes/runs/${encodeURIComponent(runId)}/retry`);
      toast({
        tone: "success",
        title: "Receita reenfileirada",
        description: "Os nós concluídos foram preservados.",
      });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível repetir",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  if (!runId) return null;

  if (loading && !data) {
    return (
      <PageBody>
        <PageHeader title="Execução de receita" />
        <p className="flex items-center gap-2 text-[13px] text-zinc-500">
          <Loader2 size={14} className="animate-spin" />
          Carregando execução…
        </p>
      </PageBody>
    );
  }

  if (error && !data) {
    return (
      <PageBody>
        <PageHeader title="Execução de receita" />
        <EmptyState
          icon={RotateCcw}
          title="Não foi possível abrir a execução"
          description={error}
          action={
            <Button size="sm" onClick={() => void load()}>
              Tentar novamente
            </Button>
          }
        />
      </PageBody>
    );
  }

  const run = data?.run;
  if (!run) return null;

  return (
    <PageBody>
      <PageHeader
        title="Execução de receita"
        description={`${run.recipe_id} · ${run.id.slice(0, 12)}…`}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void load()}>
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
            {["RUNNING_STEP", "RUNNING_SINKS", "WAITING_HITL"].includes(run.status) ? (
              <Button
                variant="outline"
                size="sm"
                disabled={busy === "cancel" || run.cancel_requested}
                onClick={() => void cancelRun()}
              >
                Cancelar
              </Button>
            ) : null}
            {["FAILED", "REJECTED", "TIMED_OUT"].includes(run.status) ? (
              <Button
                variant="outline"
                size="sm"
                disabled={busy === "retry"}
                onClick={() => void retryRun()}
              >
                Repetir
              </Button>
            ) : null}
            <Button
              variant="outline"
              size="sm"
              onClick={() => navigate("/recipes")}
            >
              <ArrowLeft size={13} />
              Receitas
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Badge variant={recipeRunStatusTone(run.status)}>
              {recipeRunStatusLabel(run.status)}
            </Badge>
            <span className="font-mono text-[13px] text-zinc-400">{run.recipe_id}</span>
            {run.dry_run ? <Badge variant="purple">dry-run</Badge> : null}
          </CardTitle>
          <span className="text-[12.5px] text-zinc-500">
            {relativeTime(run.started_at)}
            {run.completed_at ? ` · fim ${relativeTime(run.completed_at)}` : ""}
          </span>
        </CardHeader>
        <CardContent className="grid gap-3 text-[13px] sm:grid-cols-2">
          <div>
            <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">Passo atual</p>
            <p className="mt-0.5 font-mono text-zinc-300">{run.current_step_id ?? "—"}</p>
          </div>
          <div>
            <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">Pasta de saída</p>
            <p className="mt-0.5 break-all font-mono text-zinc-300">{run.output_dir_path}</p>
          </div>
          {run.error_message ? (
            <div className="sm:col-span-2">
              <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">Erro</p>
              <p className="mt-0.5 whitespace-pre-wrap text-red-300">{run.error_message}</p>
            </div>
          ) : null}
          <div className="sm:col-span-2">
            <p className="text-[11.5px] uppercase tracking-wide text-zinc-500">Gatilho</p>
            <pre className="mt-1 max-h-48 overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800 bg-zinc-950/50 p-2 font-mono text-[11.5px] text-zinc-400">
              {JSON.stringify(parseJson(run.trigger_context_json), null, 2)}
            </pre>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-[13px]">Linha do tempo da receita</CardTitle>
        </CardHeader>
        <CardContent className="space-y-1.5">
          {(data?.nodes ?? []).length === 0 ? (
            <p className="text-[12.5px] text-zinc-500">
              Sem nós persistidos (run antigo ou dry-run).
            </p>
          ) : (
            (() => {
              const nodes = data?.nodes ?? [];
              const byPath = new Map(
                nodes.map((node) => [node.node_path ?? node.node_id, node]),
              );
              const children = new Map<string, RecipeRunNode[]>();
              const roots: RecipeRunNode[] = [];
              for (const node of nodes) {
                const parent = node.parent_path;
                if (parent && byPath.has(parent)) {
                  children.set(parent, [...(children.get(parent) ?? []), node]);
                } else {
                  roots.push(node);
                }
              }
              for (const list of children.values()) {
                list.sort((left, right) => left.node_index - right.node_index);
              }
              roots.sort((left, right) => left.node_index - right.node_index);
              return roots.map((node) => (
                <RecipeNodeTree
                  key={node.id}
                  node={node}
                  children={children}
                  depth={0}
                />
              ));
            })()
          )}
        </CardContent>
      </Card>

      {data && data.hitl.length > 0 ? (
        <Card className="border-amber-500/30 bg-amber-500/[0.03]">
          <CardHeader>
            <CardTitle className="text-amber-100">Checkpoints pendentes</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {data.hitl.map((request) => {
              const options = (() => {
                try {
                  const value = JSON.parse(request.options_json);
                  return Array.isArray(value) ? (value as string[]) : [];
                } catch {
                  return [];
                }
              })();
              return (
              <div key={request.token} className="space-y-2 rounded-lg border border-amber-500/20 p-3">
                <p className="text-[13px] font-medium text-amber-100">
                  {request.title ?? "Checkpoint"}
                </p>
                <p className="whitespace-pre-wrap text-[13px] text-zinc-300">
                  {request.question_markdown ?? "Responda para continuar."}
                </p>
                <div className="flex flex-wrap gap-1.5">
                  <Button
                    size="sm"
                    disabled={busy === request.token}
                    onClick={() => void resolveHitl(request, "APPROVED")}
                  >
                    <CheckCircle2 size={12} />
                    Aprovar
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    className="border-red-500/30 text-red-300"
                    disabled={busy === request.token}
                    onClick={() => void resolveHitl(request, "REJECTED")}
                  >
                    <XCircle size={12} />
                    Rejeitar
                  </Button>
                  {options.map((option, index) => (
                    <Button
                      key={`${request.token}:${index}`}
                      size="sm"
                      variant="outline"
                      disabled={busy === request.token}
                      onClick={() => void resolveHitl(request, "CHOICE", index, option)}
                    >
                      {option}
                    </Button>
                  ))}
                </div>
              </div>
              );
            })}
          </CardContent>
        </Card>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle>Saídas por passo</CardTitle>
        </CardHeader>
        <CardContent>
          <pre className="max-h-[480px] overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800 bg-zinc-950/50 p-3 font-mono text-[11.5px] text-zinc-400">
            {JSON.stringify(parseJson(run.step_outputs_json), null, 2)}
          </pre>
        </CardContent>
      </Card>
    </PageBody>
  );
}
