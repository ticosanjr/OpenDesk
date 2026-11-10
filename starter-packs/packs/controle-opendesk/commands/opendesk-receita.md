---
description: Cria ou revisa uma receita YAML do OpenDesk.
agent: build
---
Trabalhe a receita descrita abaixo.

1. Se já existir, leia com `recipe_list` e preserve fluxo e HITL.
2. Se for nova, escreva YAML válido com `id`, `name`, `enabled: false`,
   `triggers` e `flow`.
3. Salve com `recipe_save`. Receita nova fica desligada automaticamente.
4. Valide com `recipe_run` e `dry_run=true`.
5. Só ligue com `recipe_set_enabled` após revisão humana explícita.

Nunca remova um checkpoint HITL para “simplificar” a receita.

Pedido: $ARGUMENTS
