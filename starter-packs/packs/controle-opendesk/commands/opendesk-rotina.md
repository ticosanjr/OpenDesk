---
description: Cria uma rotina desligada no OpenDesk.
agent: build
---
Crie uma rotina no workspace atual a partir do pedido abaixo.

1. Converta a recorrência para `recurrence` (`frequency`, `time`, `weekdays`
   quando necessário).
2. Escolha `target_kind` RECIPE (com `recipe_id`) ou PROMPT (com
   `command_or_prompt`).
3. Chame `routine_create`. A rotina nasce **desligada** por definição.
4. Valide com `routine_run` e `dry_run=true` quando possível.
5. Mostre a próxima execução e diga que a ativação precisa de pedido explícito.

Nunca ative a rotina sem o usuário pedir.

Pedido: $ARGUMENTS
