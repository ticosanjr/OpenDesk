# ADR 0024 — Resiliência de fila, atenção, cron seguro e entregáveis

- **Status:** aceito
- **Data:** 2026-11-07
- **Contexto:** a fila não detectava jobs travados; o Dashboard não mostrava
  agentes/tarefas bloqueadas; rotinas perdidas por suspensão podiam atrasar sem
  política; artefatos não tinham pin; não havia memória de workspace, fork de
  tarefa, dry-run de cron nem visão de model mix/economia.

## Decisão

### Fila
- `execution_queue` ganhou `lease_expires_at` e `last_heartbeat_at`.
- `claim` define lease de 60s; cada job em execução atualiza heartbeat a cada
  5s.
- Lease expirado é interrompido e vira falha, com tarefa `BLOCKED` e sem retry
  automático genérico.

### Atenção
- Dashboard ganhou `AttentionCard` com:
  - tarefas/planos aguardando aprovação com botão Aprovar;
  - tarefas BLOCKED;
  - runs FAILED nas últimas 24h com link para logs.
- API `/api/attention`.

### Cron
- `schedules.misfire_policy` (`RUN_IMMEDIATELY`/`SKIP_TO_NEXT`).
- Se o slot estiver mais de 90s atrasado, a política decide entre executar ou
  pular, cobrindo boot/suspensão.
- `safe_readonly_mode`: prompt restringe escrita à pasta de saída da rotina.
- `POST /api/schedules/{id}/dry-run`: valida prompt, pasta, engine, SMTP e
  Google sem executar o agente.
- UI de rotina expõe política, modo seguro, dry-run, histórico, duplicação e
  teste de notificação.

### Entregáveis e memória
- `artifacts_provenance.pinned`; estrela na lista/detalhe de Artefatos.
- `MEMORY.md` em `.opencode/`; endpoint para fixar resultado e injeção nos
  prompts de tarefa/rotina.
- Fork de tarefa (`forked_from_task_id`) cria card independente.

### Finanças
- `/api/cost/mix` calcula proporção por modelo, custo total, baseline no modelo
  mais caro e economia gerada pelo roteamento.
- Dashboard mostra barra de model mix e economia.

## Consequências

- Jobs travados não ficam para sempre em execução.
- O usuário vê rapidamente o que está bloqueado/aguardando.
- Rotinas perdidas respeitam uma política explícita.
- Entregáveis oficiais podem ser fixados e regras do workspace reaproveitadas.
- O valor do roteador fica visível em economia.
