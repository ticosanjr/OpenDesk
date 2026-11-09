# ADR 0038 — Governança de agentes em SQLite, com execução no OpenCode

## Contexto

O OpenDesk já tinha agentes nativos, tasks, fila, budgets por workspace e
propostas HITL, mas não uma organização viva: hierarquia, metas, budgets por
agente, quota de concorrência, heartbeats e delegação entre agentes.

## Decisão

- A definição do agente continua no arquivo nativo do OpenCode. Hierarquia,
  cargo, time, budget, quota, metas, heartbeats e delegações vivem em tabelas
  operacionais no SQLite (`agent_profiles`, `agent_goals`,
  `agent_delegations`).
- **Budget por agente** é mensal e verificado antes do dispatch. O gasto vem de
  `task_runs.estimated_cost_usd` no mês corrente.
- **Quota de concorrência** é aplicada na seleção da fila (`next_queued`):
  sem perfil, vale o limite do grupo; com perfil, vale o menor entre grupo e
  quota do agente.
- **Delegação** é sempre HITL: o pedido nasce `PENDING`, e só ao aprovar vira
  tarefa com `origin = DELEGATION`, atribuída ao agente destino e despachada.
- **Heartbeat** é opt-in por agente. Um loop de 60s cria uma tarefa de revisão
  de metas no intervalo configurado, respeitando budget e dedupe dos runs.
  O slot é reservado antes da criação para evitar disparo duplo.
- **Performance** é derivada dos últimos 30 dias de `task_runs`, não
  armazenada em uma tabela separada.

## Consequências

- Governança é auditável e não interfere no formato nativo dos agentes.
- Budget bloqueia dispatch com erro acionável; quota espera na fila em vez de
  falhar.
- Automação de heartbeat é explícita por agente e pode ser desligada a qualquer
  momento.
- Delegações sem aprovação não consomem modelo nem criam tarefas.
- O painel funciona offline: heartbeat cria a tarefa e a deixa em TODO quando o
  motor não está disponível.
