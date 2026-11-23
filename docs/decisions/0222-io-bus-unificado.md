# 0222 — Barramento I/O unificado e novos adaptadores

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** os tipos de `io_inbound_sources` e `io_outbound_sinks` estão
  fechados em CHECK constraints da migration `0001`. Faltam HTTP/RSS poller na
  entrada, HTTP webhook na saída, destino explícito (`Task`/`Recipe`/`Ticket`) e
  um lugar único para o estado de deduplicação. O disparo de receitas por pasta
  e webhook também está duplicado entre `recipes/runtime` e `io_bus/inbound`.

## Decisão

1. A migration `0002` reconstrói `io_inbound_sources`, `io_outbound_sinks` e
   `task_sink_bindings` na ordem filha → pais → rename, mantendo as FKs válidas
   dentro da transação do sqlx.
2. Novos tipos:
   - entrada: `HTTP_POLLER` (RSS/Atom/JSON/HTML);
   - saída: `HTTP_WEBHOOK`, `WHATSAPP_MSG`, `WEBPUSH` e `RSYNC_FS`;
   - `SIGNAL_MSG` entra no CHECK, que já era usado pelo código.
3. `target_kind` (`TASK`, `RECIPE`, `TICKET`, `SCHEDULE`) e `target_id`
   explicitam o destino. `config_json` com `recipe_id`/`schedule_id` continua
   aceito como fallback e é migrado para as novas colunas.
4. `io_poller_state` guarda `ETag`, `Last-Modified`, último poll e último erro;
   `io_poller_seen` deduplica por `(source_id, item_key)` e hash de conteúdo.
5. O webhook inbound valida HMAC-SHA256 com chave do Vault, com fallback para o
   `SecretStore` legado. Sem segredo configurado, a requisição é recusada.
6. O `http_webhook` outbound envia `X-OpenDesk-Idempotency-Key`, rejeita redes
   privadas/metadata por padrão e revalida o destino após a resolução DNS.
7. O watcher de pasta vira uma primitiva compartilhada por `dropzone` e pelo
   gatilho `folder` de receitas, eliminando o watcher paralelo.

## Consequências

- Adicionar um novo tipo de I/O deixa de exigir outra reconstrução de tabela.
- A UI passa a ter formulários tipados por tipo, sem depender de `config_json`
  cru para os fluxos novos.
- O poller é resiliente a XML/HTML inválido e não derruba o runtime.
- A migration é coberta por um teste que parte do schema `0001`.
