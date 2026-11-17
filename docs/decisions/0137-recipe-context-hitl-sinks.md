# 0137 — A2: HITL e sinks no `RecipeContext`

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o `RecipeContext` criado em A1 carregava apenas pool, paths e
  secrets. HITL e sinks ainda recebiam `Arc<AppState>`, o que amarrava a receita
  ao shell e impedia mover o engine para o core.

## Decisão

1. `RecipeContext` agora também carrega:
   - `events: EventBus`;
   - `dispatcher: Arc<dyn RecipeStepDispatcher>`;
   - portas opcionais `channels` e `sinks`, com implementações no-op por padrão.
2. As portas ficam no core:
   - `HitlChannelDispatcher` recebe `HitlChannelRequest`;
   - `RecipeSinkExecutor` recebe `FlowSink`, `RecipeRun`, trigger e outputs.
3. `crate::app_state::recipe_context(&Arc<AppState>)` monta o contexto e injeta
   o `ShellRecipeDispatcher`.
4. `recipes/hitl/{mod,email,signal,telegram}.rs` e `recipes/sinks.rs` passam a
   receber `&RecipeContext`; não importam mais `AppState` nem `Arc<AppState>`.
5. Telegram ganhou `send_to_allowlist_parts`/`token_for_parts`, que operam em
   pool/secrets para não exigir o estado Tauri.
6. Chamadas em engine, DAG, API, Signal, Telegram e IMAP montam o contexto no
   ponto de entrada.

## Consequências

- O caminho de HITL/sinks está pronto para o engine/DAG saírem do shell em A3.
- O comportamento externo não mudou: os mesmos canais, tokens e sinks são
  usados; apenas a dependência de tipo foi reduzida.
- A montagem do contexto é barata (clones de handles), mas chamadas dentro de
  loops ainda a recriam; A3 deve manter um contexto por execução.
- Core soma 235 testes; shell 357 (592 no total).
