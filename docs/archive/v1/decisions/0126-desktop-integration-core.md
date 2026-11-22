# 0126 — Desktop integration no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `desktop_integration` só dependia de error/paths e comandos do SO.

## Decisão

1. `desktop_integration` migra para `opendesk-core`.
2. Shell reexporta; chamadas de boot/doctor/API continuam iguais.
3. Core soma 219 testes; shell 364 (583 total).

## Consequências

- Integração de ícone/atalho AppImage é compartilhável com daemon/CLI.
- Restam engine de receitas, fila/api, integrações e cascas.
