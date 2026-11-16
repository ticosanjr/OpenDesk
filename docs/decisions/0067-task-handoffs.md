# 0067 — Handoffs de tarefa e empresa (onda 4)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** a governança já tinha organograma, metas, budgets, quotas e
  delegações simples (`agent_delegations`). Faltava um contrato durável para
  trabalho delegado entre agentes, com aceite humano, escopo e orçamento.

## Decisão

1. **Contrato explícito.** Um handoff registra origem, destino, objetivo,
   artefatos de entrada, schema de saída, critérios de aceite e sub-orçamento.
2. **Ciclo de estados.** `PENDING → ACCEPTED → RUNNING → DONE|FAILED`, com
   `REJECTED`/`CANCELLED`. Transições inválidas retornam `409`, não são
   silenciadas.
3. **Aceite humano.** Aceitar cria uma tarefa para o agente destino
   (`origin=DELEGATION`, `parent_task_id` da tarefa de origem), com o contrato
   renderizado no prompt. Rejeitar registra o motivo.
4. **Guard de orçamento.** Se `sub_budget_usd` estourar o limite diário/mensal
   ou o hard stop estiver ativo, o aceite exige `force=true` — e a UI pede uma
   segunda confirmação explícita.
5. **Conclusão automática.** Quando o run da tarefa gerada termina,
   `complete_for_task` fecha o handoff como `DONE`/`FAILED`.
6. **MCP.** `handoff_create`, `handoff_list`, `handoff_respond` e
   `handoff_finish` permitem que agentes proponham e respondam delegações sem
   sair do OpenCode.
7. **UI.** Governança ganhou “Handoffs de tarefa”, separada das delegações
   simples, com aceite/rejeição/conclusão e link para a tarefa.

## Consequências

- Delegações entre agentes ficam auditáveis e orçadas, não são mensagens soltas.
- O handoff é a base para a empresa multiagente das próximas ondas (workers,
  memória compartilhada e MCP Hub).
