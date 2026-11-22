# 0088 — Write-buffer para heartbeats e presença

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** cada run envia heartbeat a cada 5s e cada nó remoto atualiza
  presença com a mesma frequência. Com execuções concorrentes, esses `UPDATE`
  idempotentes competem com a API, o watcher e o worker da fila pelo único
  escritor do SQLite, podendo produzir `SQLITE_BUSY` e latência.

## Decisão

1. Todo heartbeat de job (`execution_queue.last_heartbeat_at` e lease) e toda
   atualização de presença de nó (`workers.last_seen_at`) passam por
   `db::write_buffer::WriteBuffer`.
2. O buffer usa um canal mpsc limitado e **um único task escritor**. Quando o
   canal enche, o produtor aguarda (backpressure) em vez de falhar ou descartar.
3. O escritor acumula até 250 ms (ou até o shutdown) e executa o lote em uma
   transação `BEGIN IMMEDIATE` na conexão dedicada.
4. Atualizações repetidas para o mesmo job/nó são coalescidas por chave; só a
   última importa, pois o efeito é "tocar" um timestamp.
5. Transições de estado não são bufferizadas: `RUNNING`, `SUCCESS`, `FAILED`,
   `OFFLINE`, `BUSY`, `DRAINING` e mudanças de capabilities continuam escritas
   imediatas para que a API e a UI as observem sincronamente.
6. Contadores (`queued`, `coalesced`, `flushed`, `failed`, `dropped`,
   `pending`) aparecem em `/api/about` para diagnóstico.
7. `AppState` encerra o buffer no shutdown normal; no `Drop` o canal fechado
   ainda drena o que já estava enfileirado.

## Consequências

- Rajadas de heartbeat deixam de multiplicar transações: 1.200 atualizações
  concorrentes (4 runs + 8 nós) viram poucos lotes, sem falha de lock.
- Um heartbeat pode atrasar até 250 ms; leases de 60s e a tolerância de
  presença absorvem isso com folga.
- O writer dedicado reduz a contenção, mas não substitui um backend Postgres
  para HA multi-processo. O lock de instância (ADR 0087) continua necessário.
