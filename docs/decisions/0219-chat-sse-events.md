# 0219 — Chat event-driven via SSE de sessão

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** `chat::wait_for_turn` consultava `/api/session/{id}/message` a
  cada 500 ms até ver idle + texto. O daemon já expõe `/api/event` e o
  `session_runner` já consumia esses eventos; o chat era o último polling
  constante do caminho quente.

## Decisão

1. `opencode_client::event_stream` centraliza o parser SSE
   (`extract_events`, `extract_events_bytes`, `session_matches`); o
   `session_runner` e o `RestClient` usam o mesmo parser.
2. `RestClient::event_receiver` abre `/api/event` e entrega `serde_json::Value`
   por `mpsc`, preservando blocos incompletos entre chunks TCP.
3. `send_message_with_context` assina o stream **antes** de enviar o prompt,
   evitando perder um `session.idle` imediato.
4. `wait_for_turn`:
   - acorda em `session.idle`, `session.execution.succeeded` ou
     `session.execution.failed` da sessão correta;
   - ignora eventos de outras sessões;
   - faz uma leitura de confirmação e exige texto do turno atual;
   - mantém fallback de 10 s quando há SSE e 500 ms quando o stream não está
     disponível (testes e degradação), além do fallback de estabilidade de 30 s;
   - respeita `CancellationToken`.
5. Timeout global configurável por `chat_turn_timeout_minutes` (padrão 30,
   limitado a 1–180); ao expirar, retorna erro explícito em vez da resposta
   anterior.
6. O parsing puro (usage, sinais de turno, `parse_assistant_since`,
   `parse_model_ref`) mudou para `chat_parse.rs`; `chat.rs` mantém a orquestração.
7. Testes cobrem entrega imediata (<500 ms), evento de outra sessão ignorado,
   timeout sem turno final e o caminho antigo sem SSE.

## Consequências

- O chat deixa de fazer polling de 500 ms no caminho normal.
- O parser SSE não está duplicado.
- Eventos de tool-only continuam sem encerrar o turno.
- Shell: **352 testes**; engine: **134**.
