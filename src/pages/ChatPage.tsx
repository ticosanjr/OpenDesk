import { useState } from "react";
import {
  ArrowUp,
  Bot,
  Loader2,
  MessageSquare,
  Paperclip,
  Route,
  Sparkles,
  Wand2,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiPost } from "@/lib/api";
import type { DispatchDecision, Task } from "@/lib/types";
import { Link } from "react-router-dom";

const CAPABILITIES = [
  {
    icon: MessageSquare,
    title: "Chat com streaming",
    description:
      "Sessões do OpenCode com texto, raciocínio e tool calls transmitidos em tempo real.",
  },
  {
    icon: Wand2,
    title: "Meta-Builder",
    description:
      "“Crie um subagente revisor fiscal…” gera o arquivo .opencode/agents/<slug>.md validado e um card pronto para usar.",
  },
  {
    icon: Route,
    title: "Roteador DSH",
    description:
      "Classifica a intenção, decide Plan vs Build, escolhe subagente, modelo e effort antes de executar.",
  },
  {
    icon: Sparkles,
    title: "Context Packer",
    description:
      "Injeta somente caminhos, skills e restrições relevantes, incluindo a regra obrigatória de output.",
  },
];

export function ChatPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("AUTO_DSH");
  const [sending, setSending] = useState(false);
  const [decision, setDecision] = useState<DispatchDecision | null>(null);
  const [created, setCreated] = useState<Task | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [metaPrompt, setMetaPrompt] = useState("");
  const [metaBusy, setMetaBusy] = useState(false);
  const [metaResult, setMetaResult] = useState<{
    kind: string;
    id: string;
    path: string;
    message: string;
  } | null>(null);
  const [metaError, setMetaError] = useState<string | null>(null);

  async function buildMeta() {
    if (!activeWorkspaceId) {
      setWorkspaceDialogOpen(true);
      return;
    }
    if (!metaPrompt.trim()) {
      setMetaError("Descreva o agente, skill ou comando.");
      return;
    }
    setMetaBusy(true);
    setMetaError(null);
    try {
      const result = await apiPost<{
        kind: string;
        id: string;
        path: string;
        message: string;
      }>(
        `/api/meta/build?workspace_id=${encodeURIComponent(activeWorkspaceId)}&scope=workspace`,
        { prompt: metaPrompt.trim() },
      );
      setMetaResult(result);
      setMetaPrompt("");
    } catch (cause) {
      setMetaError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setMetaBusy(false);
    }
  }

  async function submit() {
    if (!activeWorkspaceId) {
      setWorkspaceDialogOpen(true);
      return;
    }
    if (!prompt.trim()) {
      setError("Escreva uma tarefa.");
      return;
    }
    setSending(true);
    setError(null);
    try {
      const title =
        prompt.trim().split("\n")[0].slice(0, 90) || "Tarefa rápida";
      const task = await apiPost<Task>(
        `/api/tasks?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        {
          title,
          description: prompt.trim(),
          execution_mode: mode,
          priority: 2,
          tags: [],
          effort_level: "medium",
        },
      );
      const response = await apiPost<{ decision: DispatchDecision }>(
        `/api/tasks/${task.id}/dispatch`,
        {},
      );
      setDecision(response.decision);
      setCreated(task);
      setPrompt("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSending(false);
    }
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="interação direta"
        title="Chat & Criador"
        description="Envie uma tarefa rápida; o roteador DSH decide a fase, o agente e o modelo, e a fila executa."
        actions={<Badge variant="blue">fila integrada</Badge>}
      />

      <div className="grid gap-3 md:grid-cols-2">
        {CAPABILITIES.map((capability) => (
          <Card key={capability.title}>
            <CardContent className="p-4">
              <div className="flex items-start gap-3">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/60 text-zinc-500">
                  <capability.icon size={15} strokeWidth={1.7} />
                </div>
                <div>
                  <p className="text-[12px] font-medium text-zinc-200">
                    {capability.title}
                  </p>
                  <p className="mt-1 text-[11px] leading-relaxed text-zinc-500">
                    {capability.description}
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Wand2 size={15} className="text-zinc-500" />
              Meta-Builder
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Crie um agente, skill ou comando descrevendo o pedido em linguagem natural. O arquivo
              nativo é gravado no workspace e validado pelo Studio.
            </p>
          </div>
          <Badge variant="purple">disco</Badge>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-end gap-2">
            <textarea
              value={metaPrompt}
              onChange={(event) => setMetaPrompt(event.target.value)}
              rows={2}
              placeholder="Crie um subagente chamado revisor-fiscal focado em contabilidade italiana, usando o modelo deepseek/deepseek-flash, que só pode ler arquivos."
              className="min-h-[52px] flex-1 resize-y rounded-lg border border-zinc-800 bg-zinc-950/50 px-3 py-2 text-[11px] text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-blue-500/60"
            />
            <Button
              size="sm"
              className="h-9 shrink-0"
              onClick={() => void buildMeta()}
              disabled={metaBusy}
            >
              {metaBusy ? <Loader2 size={14} className="animate-spin" /> : <Wand2 size={14} />}
              Criar arquivo
            </Button>
          </div>
          {metaError ? (
            <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
              {metaError}
            </p>
          ) : null}
          {metaResult ? (
            <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/[0.06] p-3 text-[11px] text-emerald-200">
              <p className="font-medium">{metaResult.message}</p>
              <p className="mt-1 font-mono text-[9px] text-emerald-300/70">
                {metaResult.kind} · {metaResult.id} · {metaResult.path}
              </p>
              <Link
                to="/studio"
                className="mt-2 inline-block text-[10px] text-emerald-300 underline-offset-2 hover:underline"
              >
                Abrir no OpenCode Studio →
              </Link>
            </div>
          ) : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Nova tarefa rápida</CardTitle>
          <div className="flex items-center gap-2">
            <Badge variant="default">router → fila → OpenCode</Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          {decision ? (
            <div className="rounded-lg border border-blue-500/20 bg-blue-500/[0.06] p-3 text-[11px] text-blue-200">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="blue">{decision.phase}</Badge>
                <span className="font-medium">@{decision.selected_agent}</span>
                <span className="font-mono text-[10px]">{decision.selected_model}</span>
                <span className="font-mono text-[10px]">effort {decision.effort}</span>
                {decision.requires_plan_approval ? (
                  <Badge variant="yellow">aprovação necessária</Badge>
                ) : null}
              </div>
              <p className="mt-1 text-blue-300/80">{decision.rationale}</p>
              <p className="mt-1 font-mono text-[9px] text-blue-300/60">
                output: {decision.output_directory}
              </p>
              {created ? (
                <Link
                  to="/kanban"
                  className="mt-2 inline-block text-[10px] text-blue-300 underline-offset-2 hover:underline"
                >
                  Abrir no Kanban →
                </Link>
              ) : null}
            </div>
          ) : null}

          {error ? (
            <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
              {error}
            </p>
          ) : null}

          {!activeWorkspaceId ? (
            <EmptyState
              icon={MessageSquare}
              title="Nenhum workspace ativo"
              description="Crie um workspace para enfileirar tarefas."
              action={
                <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
                  Criar workspace
                </Button>
              }
            />
          ) : (
            <>
              <div className="flex flex-wrap items-center gap-2">
                <select
                  value={mode}
                  onChange={(event) => setMode(event.target.value)}
                  className="h-8 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2 text-[10px] text-zinc-300 outline-none"
                >
                  <option value="AUTO_DSH">AUTO_DSH — roteador decide</option>
                  <option value="PLAN_ONLY">PLAN_ONLY — só plano</option>
                  <option value="PLAN_THEN_BUILD">PLAN_THEN_BUILD — aprovar plano</option>
                  <option value="BUILD_DIRECT">BUILD_DIRECT — executar direto</option>
                </select>
                <Badge variant="default">orçamento e hard cap aplicados</Badge>
              </div>
              <div className="flex items-end gap-2 rounded-xl border border-zinc-800 bg-zinc-950/50 px-3 py-2">
                <Paperclip size={15} className="mb-2 text-zinc-600" />
                <textarea
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                  onKeyDown={(event) => {
                    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                      event.preventDefault();
                      void submit();
                    }
                  }}
                  rows={3}
                  placeholder="Descreva a tarefa. Cmd/Ctrl+Enter envia para o roteador e a fila…"
                  className="min-h-[64px] flex-1 resize-y bg-transparent py-1 text-[12px] text-zinc-200 outline-none placeholder:text-zinc-600"
                />
                <Button
                  size="icon"
                  className="h-8 w-8"
                  onClick={() => void submit()}
                  disabled={sending}
                >
                  {sending ? (
                    <Loader2 size={15} className="animate-spin" />
                  ) : (
                    <ArrowUp size={15} />
                  )}
                </Button>
              </div>
              <p className="flex items-center gap-1.5 text-[9px] text-zinc-600">
                <Bot size={11} />
                Cria a tarefa no Kanban, aplica o roteador DSH e enfileira no SQLite. O progresso
                aparece em Logs.
              </p>
            </>
          )}
        </CardContent>
      </Card>
    </PageBody>
  );
}
