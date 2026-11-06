# ADR 0014 — Overrides de tarefa e saúde de modelos

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** editar modelo/agente no Kanban não alterava a execução sob
  políticas automáticas, e o router continuava escolhendo modelos bloqueados
  por região (`403 This model is not available in your country`).

## Decisão

- Novas colunas `agent_override` e `model_override` em `tasks`.
  - Só valores marcados como override vencem a política/regra.
  - O router não grava mais agente/modelo na tarefa; esses campos representam
    apenas escolha explícita do usuário.
- A política/regra escolhe o modelo apenas quando não há override.
- `model_health` registra falhas `model_unavailable`/`auth_failed`.
  - Modelos em cooldown de 24h são excluídos do router.
  - Sucesso limpa o cooldown.
- Transições do Kanban persistem a coluna antes de tentar executar.
  - Falha de dispatch mantém a coluna e grava `phase_status=dispatch_failed`.
- O seletor de modelo do modal de edição é um combobox com filtro por texto.

## Consequências

- Editar o modelo de uma tarefa passa a afetar a próxima execução.
- Tarefas antigas com agente/modelo auto-preenchidos não são tratadas como
  override até o usuário salvar a edição.
- Um modelo bloqueado deixa de ser escolhido automaticamente após a primeira
  falha.
- Mover um card nunca “volta sozinho” por causa de motor offline.
