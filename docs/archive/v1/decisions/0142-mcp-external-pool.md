# 0142 — E2: pool de servidores MCP externos

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o gateway MCP expunha apenas as tools do OpenDesk; servidores
  declarados em `mcp.servers` não apareciam no endpoint.

## Decisão

1. `opendesk_core::mcp_pool` lê `mcp.servers` global + workspace via
   `jsonc_editor`; a entrada do workspace vence por nome.
2. Servidores `local` são processos stdio line-delimited JSON-RPC:
   - handshake `initialize` + `notifications/initialized`;
   - `tools/list` no connect;
   - chamadas serializadas, timeout de 30s e uma reconexão após erro.
3. Servidores `remote` usam JSON-RPC sobre HTTP, com headers e timeout.
4. Tools externas são expostas como `server__tool`; `split_namespaced` roteia a
   chamada para o servidor correto.
5. `handle_http_with` agrega as tools externas e encaminha chamadas
   namespaced; falha de um servidor não derruba as tools locais.
6. Testes cobrem namespace, config com `disabled`, um servidor fake e dois
   servidores com a mesma tool.

## Consequências

- Um único `/api/mcp` combina tools locais, receitas/agentes e MCPs externos.
- O pool é serializado por servidor; paralelismo entre servidores é natural,
  mas chamadas simultâneas ao mesmo processo esperam a vez.
- Servidores remotos com OAuth/streaming avançado ainda precisam de uma
  integração dedicada.
