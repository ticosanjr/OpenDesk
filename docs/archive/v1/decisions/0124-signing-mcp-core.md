# 0124 — Assinatura de update e loop MCP no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `update_signing` e `mcp_server` não dependiam do shell.

## Decisão

1. `update_signing` e `mcp_server` migram para `opendesk-core`.
2. A chave pública `update-signing-public-key.txt` migra junto.
3. Core ganha `ed25519-dalek`.
4. Shell reexporta; release/MCP stdio seguem usando os mesmos símbolos.
5. Core soma 216 testes; shell 367 (583 total).

## Consequências

- Assinatura de update e loop JSON-RPC podem ser usados por daemon/CLI.
- Restam engine de receitas, fila/api e cascas.
