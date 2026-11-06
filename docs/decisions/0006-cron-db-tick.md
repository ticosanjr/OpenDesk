# ADR 0006 — Cron dirigido pelo SQLite

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o OpenDesk precisa de rotinas que sobrevivam a reinícios, rodem
  sem uma janela de chat e não disparem duas vezes o mesmo horário. A biblioteca
  `tokio-cron-scheduler` mantém o estado em memória; sincronizar CRUD, restart e
  relógio com o SQLite adiciona complexidade.

## Decisão

O motor cron é um loop de 15 s que lê `schedules` ativas do SQLite, calcula a
próxima ocorrência com `croner` + `chrono-tz`, e enfileira quando
`next_run_at <= now`. Cada job carrega `schedule_id` e `scheduled_for`; um índice
único parcial em `execution_queue(schedule_id, scheduled_for)` garante
idempotência.

- A pasta determinística é criada antes do enqueue.
- `next_run_at` é recalculado a partir de agora depois do disparo, evitando uma
  tempestade de execuções atrasadas.
- `POST /api/schedules/tick` permite testar o motor sem esperar o relógio.
- O modo headless e o app compartilham o mesmo motor.

## Consequências

- O estado é auditável e sobrevive a crashes.
- A granularidade é de 15 s; isso é suficiente para rotinas de minutos/horas.
- Um job pode ficar na fila enquanto o orçamento está bloqueado; nesse caso ele
  não é perdido.
- O índice único impede duplicatas, mas um slot manual (`run now`) usa o
  timestamp atual como chave.
