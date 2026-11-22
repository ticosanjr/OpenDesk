# 0202 — Onda 3 (início): domínio de canais, MCP rate/RBAC e NotificationRouter

- **Status:** aceito como entrega parcial
- **Data:** 2026-11-22
- **Contexto:** a Onda 2 ainda não extraiu fila/runner/telemetria/DAG, e os
  adaptadores reais de canal vivem no shell acoplados a `AppState`. O pool,
  config e server do MCP dependem de `paths`, `secrets`, `events` e `db` que
  continuam no core. Para não violar as fronteiras, foram extraídas apenas as
  partes já independentes.

## Decisão

1. **Domínio de canais:** `ChannelAdapter`, `NormalizedEnvelope`, `Attachment`,
   `ChannelThread`, `ChannelCapabilities`, `DeliveryReceipt` e `dedupe_key`
   derivado de canal/ID/thread. Nenhum nome de provedor no engine/recipes.
2. **Política de notificação:** `CareLevel` e `QuietHours` no domínio (janela
   que cruza meia-noite, validação 0..23).
3. **MCP:** `mcp_rate` e `mcp_rbac` (puros) foram para `opendesk-mcp` como
   `rate`/`rbac`; o core reexporta os módulos. O lock do rate limiter deixou de
   usar `expect` e recupera de envenenamento.
4. **`opendesk-channels`:** features `ch-telegram`, `ch-whatsapp`, `ch-email`,
   `ch-signal`, `ch-webpush`; `enabled_channels()` registra descritores por
   feature e `NotificationRouter` decide entrega/adiamento por care level,
   quiet hours e severidade (crítico ignora quiet hours).
5. **Elicitation/sampling:** ports `ElicitationPort` e `SamplingPort` no
   domínio; a ligação com HITL 2.0 e execução será feita quando o MCP server
   sair do core.
6. Verificado que `opendesk-engine`/`opendesk-recipes` não citam telegram,
   whatsapp, signal, webpush ou smtp.

## Consequências

- A política de notificação já está desacoplada dos adaptadores e testada.
- `cargo check -p opendesk-channels --no-default-features` e
  `cargo test --all-features` passam; os adaptadores de I/O continuam no shell
  até a extração de `AppState`/engine.
- Testes: 340 shell + 264 core + 27 domain + 6 engine + 3 mcp + 13 recipes +
  15 storage + 4 channels (**672**).
- **Pendente da Onda 3:** mover `mcp_config`, `mcp_pool` (split em pool/
  transportes), `mcp_server` e os adaptadores reais de canal; RBAC/rate/auditoria
  no ponto de chamada do pool; integração de elicitation/sampling. Tudo depende
  de concluir a extração de `db`/`paths`/`secrets` e o restante da Onda 2.
- Cobertura de 75% segue não atingida; o `coverage-core.sh` ainda mede o escopo
  antigo.
