# 0151 — D3: lease expirada e retomada entre nós

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** se um worker remoto morresse no meio de um job, o job ficava
  `RUNNING` para sempre e nenhum outro nó o retomava.

## Decisão

1. `queue::requeue_expired` roda antes de `next-job`:
   - lease vencida e `retry_count < max_retries` → volta para `QUEUED`, limpa
     `assigned_node_id` e incrementa `retry_count`;
   - sem retries → `FAILED` com mensagem acionável.
2. O claim continua atômico (`WHERE status = 'QUEUED'`), então dois nós não
   pegam o mesmo job.
3. Teste com dois nós:
   - nó A pega o job;
   - lease é forçada para o passado;
   - nó B pega **o mesmo job** com `retry_count = 1`;
   - a tabela continua com uma única linha.
4. O teste de dois containers/máquinas reais fica como validação manual; o
   protocolo e a ausência de duplicação estão cobertos localmente.

## Consequências

- Um restart de worker não perde trabalho nem cria job duplicado.
- A retomada é por job, não por passo; o harness precisa ser idempotente ou
  usar o workdir sincronizado.
- `max_retries` agora protege contra loops de lease.
