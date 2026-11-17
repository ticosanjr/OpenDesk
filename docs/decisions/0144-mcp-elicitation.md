# 0144 — E4: elicitation → HITL 2.0

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o gateway precisava pedir dados ao cliente MCP sem inventar
  campos ou bloquear a tool para sempre.

## Decisão

1. Nova tool `elicit` com `message`, `schema`, `task_id` e
   `timeout_seconds`.
2. Em uma sessão Streamable HTTP, a API envia
   `elicitation/create { message, requestedSchema }` pelo stream e espera a
   resposta JSON-RPC com o mesmo `id`.
3. `McpSession` mantém um mapa de requisições pendentes (`oneshot`); respostas
   do cliente são roteadas por `resolve_client_response`.
4. Sem sessão, a API usa `elicit_fallback` → `ask_user`, que cria uma interação
   HITL 2.0 e entrega pelos canais já configurados.
5. Timeout retorna erro acionável; nenhuma suposição é feita.
6. Testes cobrem a resolução de uma requisição servidor→cliente e o erro sem
   sessão/task_id.

## Consequências

- Clientes que implementam elicitation recebem a pergunta no próprio cliente.
- Clientes simples continuam usando o inbox HITL/Telegram/Signal via `task_id`.
- A tool nunca “adivinha” o valor ausente.
