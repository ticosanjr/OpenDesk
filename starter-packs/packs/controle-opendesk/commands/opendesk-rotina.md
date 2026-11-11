---
description: Cria uma rotina desligada no OpenDesk.
agent: build
---
Crie uma rotina no workspace atual a partir do pedido abaixo.

1. Para CRON, converta a recorrência para `recurrence` (`frequency`, `time`,
   `weekdays` quando necessário) ou passe `cron_expression`.
2. Para FOLDER_WATCH, RSYNC_FOLDER, IMAP_IDLE, SIGNAL_MSG, WEBHOOK ou SQL_POLL,
   use `trigger_type` + `trigger_config` (ex.: {"path":"input"}).
3. Escolha `target_kind` RECIPE (com `recipe_id`) ou PROMPT (com
   `command_or_prompt`).
4. Chame `routine_create`. A rotina nasce **desligada** por definição.
5. Valide com `routine_run` e `dry_run=true` quando possível.
6. Mostre o gatilho e diga que a ativação precisa de pedido explícito.

Nunca ative a rotina sem o usuário pedir.

Pedido: $ARGUMENTS
