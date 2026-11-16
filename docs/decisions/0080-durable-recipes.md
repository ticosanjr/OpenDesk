# 0080 — Receitas duráveis: nós, cancelamento e retomada

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** receitas rodavam em `tokio::spawn` com estado apenas em memória;
  um restart deixava runs `RUNNING_STEP` órfãos e não havia cancelar/repetir.

## Decisão

1. **Nós persistidos.** A migração 0049 cria `recipe_run_nodes` com índice,
   id, tipo, status, tentativas, sessão, output, erro e timestamps.
2. **Driver retoma do primeiro nó não concluído.** Ao iniciar ou reiniciar, o
   runtime carrega runs `RUNNING_STEP/RUNNING_SINKS/WAITING_HITL` e reinvoca o
   engine; nós `DONE` não são reexecutados.
3. **HITL idempotente.** Antes de criar um checkpoint, o engine reutiliza um
   `hitl_requests` PENDING do mesmo run/checkpoint; retomadas não duplicam a
   pergunta.
4. **Cancelamento cooperativo.** `recipe_runs.cancel_requested` é marcado por
   `POST /api/recipes/runs/{id}/cancel`; o driver verifica entre nós e durante a
   espera do run, cancela jobs ativos e finaliza como `REJECTED` com mensagem
   “cancelado pelo usuário”. Checkpoints pendentes viram `CANCELLED`.
5. **Retry.** `POST /api/recipes/runs/{id}/retry` reseta apenas nós não
   concluídos, limpa o erro e reusa o mesmo run/task.
6. **Timeline.** `GET /api/recipes/runs/{id}` devolve `nodes`; a UI mostra
   status, tentativas, erro e output por nó, com botões Cancelar/Repetir.
7. **Limite.** Paralelismo/DAG ainda não entram nesta fatia; o formato YAML
   continua linear e compatível.

## Consequências

- Derrubar o app no meio de uma receita não perde o progresso já concluído.
- O operador vê exatamente onde parou e pode cancelar ou repetir.
- A base está pronta para `parallel/map/branch` usarem os mesmos nós duráveis.
