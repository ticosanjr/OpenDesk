# 0113 — Helpers de browse no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `dsh_router` ainda dependia de `crate::browse` apenas para tags,
  allowlist e detecção de intenção de navegação. Isso impedia mover o
  `dsh_router` para o core.

## Decisão

1. Novo módulo `browse_tags` no core com:
   - `BROWSE_TAG`, `BROWSE_ALLOWLIST_PREFIX`, `HIGH_RISK_TAGS`;
   - `needs_browser`;
   - `allowlist_from_tags`;
   - `is_high_risk`.
2. O shell `browse` reexporta os helpers e mantém somente a integração com
   OpenCode Browse/MCP/skill.
3. `dsh_router` pode usar `crate::browse_tags` sem depender de `AppState`.
4. Testes de parsing/risco/intenção rodam no core; testes de ativação do
   browse continuam no shell.

## Consequências

- O próximo passo pode mover `dsh_router` para o core.
- `browse` ainda depende de `AppState` para configurar MCP/skill; isso fica no
  shell até existir um `CoreServices`.
