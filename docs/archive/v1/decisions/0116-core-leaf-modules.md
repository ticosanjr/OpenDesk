# 0116 — Módulos folha restantes no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** media, skill_eval, personal_context e run_timeline não dependiam
  de Tauri nem de integrações; continuavam no shell por inércia.

## Decisão

1. Movidos para `opendesk-core`: `media`, `skill_eval`, `personal_context` e
   `run_timeline`.
2. Shell reexporta; `crate::media`, `crate::skill_eval`,
   `crate::personal_context` e `crate::run_timeline` seguem válidos.
3. Testes usam o `test_support` do core (`ctx.db`/`ctx.paths`).
4. Core soma 168 testes; shell 415 (583 total).

## Consequências

- Avaliação de skills, contexto pessoal, mídia e timeline já podem ser usados
  pelo daemon/CLI.
- Restam `recipes`, `queue_scheduler`, `api`/`app_state`, `browse` e os binários
  separados para concluir REF-01.
