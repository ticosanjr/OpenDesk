# 0130 — Endpoint/API local e daemon probe no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `ApiEndpoint`, `internal_api` e `daemon_probe` eram compartilhados
  por CLI/MCP/shell, mas viviam no shell.

## Decisão

1. `ApiEndpoint` e `base_url` migram para `opendesk-core::api_endpoint`.
2. `AppState` reexporta `ApiEndpoint`; shell_state usa `base_url` do core.
3. `internal_api` e `daemon_probe` migram para o core.
4. Shell reexporta; CLI/MCP/shell continuam com os mesmos caminhos.
5. Core soma 228 testes; shell 355 (583 total).

## Consequências

- Cliente local e probe podem ser usados por daemon/CLI.
- Restam engine de receitas, fila/api e cascas.
