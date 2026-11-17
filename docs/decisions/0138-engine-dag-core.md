# 0138 — A3: engine e DAG no core

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o engine e o DAG ainda viviam no shell e recebiam
  `Arc<AppState>`, apesar de o protocolo de HITL já estar no core desde A2.

## Decisão

1. `recipes::engine` e `recipes::dag` passam a viver em `opendesk-core`.
2. A API do engine muda de `Arc<AppState>` para `RecipeContext`:
   - `start(context: RecipeContext, ...)`;
   - `resume(context: RecipeContext, ...)`;
   - `execute(context: &RecipeContext, ...)`.
3. O protocolo HITL (`request`, `wait`, `resolve`, `parse_reply`,
   `extract_token`) também vai para o core em `recipes::hitl`.
   - Canais concretos são entregues pela porta `HitlChannelDispatcher`.
   - O shell registra `ShellHitlChannels` no contexto.
4. Sinks são executados pela porta `RecipeSinkExecutor`; o shell registra
   `ShellRecipeSinks`, que delega ao `io_bus`.
5. O shell deixa de compilar `engine.rs`/`dag.rs` e reexporta os módulos do
   core em `recipes/mod.rs`.
6. Chamadas em API, cron, runtime e I/O bus montam o contexto com
   `crate::app_state::recipe_context(&state)`.

## Consequências

- `opendesk-core` não depende de Tauri nem de `AppState`; o engine usa apenas
  pool, paths, eventos, secrets, dispatcher, canais e sinks.
- Testes de engine/DAG foram movidos para o core (249 testes no core, 343 no
  shell; 592 no total).
- O runtime de receitas, a fila e o restante do shell continuam em A4/A5.
- O contexto é clonado por execução; a próxima fatia deve evitar recriá-lo a
  cada passo.
