# 0119 — Recurrence e cron de receitas no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `recipes::cron` dependia de `queue_scheduler::cron_engine` apenas
  para calcular a próxima ocorrência CRON.

## Decisão

1. `next_occurrence` migra para `opendesk-core::recurrence`, usando `croner` e
   `chrono-tz`.
2. `queue_scheduler::cron_engine` reexporta a função; nada muda para chamadores.
3. `recipes::cron` migra para o core e usa `crate::recurrence`.
4. Shell reexporta `recipes::cron`; o engine/sinks/HITL de canais continuam no
   shell.
5. Core soma 201 testes; shell 382 (583 total).

## Consequências

- Cálculo de recorrência é compartilhado por receitas, rotinas e calendário.
- O próximo bloqueio do engine é o dispatcher de passos (`api::tasks`).
