# 0207 — Primeiro corte do engine: módulos de baixo nível saem do core

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** `opendesk-core` tinha 29 mil linhas e misturava contratos de
  domínio com adaptadores, cliente OpenCode, router, sandbox e Studio SSOT. A
  extração precisava começar por um cluster sem ciclo de dependências.

## Decisão

Mover para `opendesk-engine` os módulos que já dependiam apenas de
domain/storage (mais os que dependem uns dos outros):

- `sandbox`, `runtime`, `token_estimate`, `media`, `text`, `browse`,
  `browse_tags`, `personal_context`, `daemon_probe`, `run_timeline`;
- `config_sync` (Studio SSOT de arquivos) e `dsh_router`;
- `adapters`, `opencode_client`, `api_endpoint`;
- portas `notifier`, `queue_ports`, `recipe_dispatch`.

O `opendesk-core` mantém `pub use opendesk_engine::{...}` para não quebrar o
shell, o daemon nem o CLI. O engine ganhou erro próprio (`AppError` com
`Daemon`/`Http`) e conversões nos dois sentidos com o erro do core, para que o
`?` continue funcionando nas bordas.

## Consequências

- `opendesk-core` cai de ~29,3 mil para ~20 mil linhas sem mudar comportamento.
- `config_sync` e `dsh_router` passam a ser reusáveis pelo daemon/CLI sem
  arrastar o core inteiro.
- A fila e as receitas ainda vivem no core; o próximo corte precisa inverter a
  dependência (porta de receita agendada) antes de movê-las para o engine.
- Test-only `test_support` foi replicado no engine; ele é ignorado pelo gate de
  `unwrap/expect`.
