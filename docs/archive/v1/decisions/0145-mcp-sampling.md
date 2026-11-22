# 0145 — E5: sampling → DSH + budget

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** servidores MCP podem pedir uma amostra de texto ao cliente;
  o OpenDesk precisava de um caminho seguro quando o cliente não implementa
  `sampling/createMessage`.

## Decisão

1. Nova tool `sampling` com `prompt`, `model`, `max_tokens` e
   `timeout_seconds`.
2. Com sessão, a API envia `sampling/createMessage` pelo stream e devolve o
   resultado do cliente.
3. Sem cliente compatível (ou timeout), `mcp_sampling::fallback`:
   - consulta `budget::check`; `hard_stop` impede a chamada;
   - carrega modelos do workspace;
   - usa `dsh_router::classify` com o prompt para escolher agente/modelo;
   - chama `RestClient::generate_text`;
   - devolve modelo, texto e status do budget.
4. A decisão é auditada como `mcp:sampling`.
5. Teste cobre o bloqueio por hard stop antes de qualquer chamada ao motor.

## Consequências

- Sampling respeita o mesmo budget dos runs.
- O caminho DSH depende do daemon OpenCode online; offline retorna erro claro.
- O consumo gerado por `generate_text` não é contabilizado como run; o limite é
  verificado antes e a chamada é auditada.
