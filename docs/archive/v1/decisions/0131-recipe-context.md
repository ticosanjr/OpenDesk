# 0131 — A1: RecipeContext no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** primeiro item da Fase A do plano novo.

## Decisão

1. Novo `opendesk_core::recipe_context::RecipeContext` com:
   - `SqlitePool`;
   - `AppPaths`;
   - `Arc<SecretStore>`.
2. Sem `AppState`, Tauri, integrações ou handlers HTTP.
3. Teste constrói o contexto com `test_support` e valida o pool/paths.
4. Nenhum call site mudou nesta fatia.

## Próximo passo (A2)

- HITL de canais e `sinks` passam a receber `&RecipeContext` em vez de
  `Arc<AppState>`.
- Shell constrói o contexto uma vez por run.
