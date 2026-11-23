# 0221 — Credential Vault zero-exposure

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** o `SecretStore` já cifra segredos de integrações com AES-256-GCM
  em arquivos `0600`, mas não existe um cofre unificado para credenciais do
  usuário (`login`, `api_key`, `totp_seed`, `header_bundle`), com ACL por
  agente/receita, auditoria e redação global. Sem isso, qualquer segredo que
  chegue ao prompt, ao stdout de um CLI ou a um artefato pode vazar para o LLM.

## Decisão

1. O Vault vive no SQLite: `vault_entries` guarda `secret_ciphertext` e
   `secret_nonce`; `vault_acls` guarda permissões por `agent`, `recipe`, `mcp` e
   `sink`; `vault_audit` registra `INJECTED`, `DENIED_ACL`, `DENIED_DOMAIN`,
   `DENIED_HITL` e `EXPIRED` sem valores em claro.
2. O payload reutiliza a chave de máquina do `SecretStore`. `secrets.rs` expõe
   `seal`/`open`, e o `SecretStore` passa a usá-los internamente.
3. Nenhuma API HTTP, tool MCP ou prompt retorna plaintext. A única leitura
   ocorre em `VaultBroker::resolve_for_edge(actor, entry_id, domain)`, em memória
   Rust, com domínio exato, expiração e ACL verificados.
4. A injeção acontece nas bordas controladas pelo OpenDesk: HTTP outbound, MCP
   Hub e o proxy local. O Browse é um MCP externo e recebe apenas handoff HITL
   para login/2FA; o OpenDesk não preenche o navegador do usuário.
5. O sandbox não recebe segredos por variável de ambiente. Quando um fluxo
   precisar de credencial, ele usa uma borda HTTP/MCP com token efêmero.
6. Um `SecretScrubber` por run mascara o valor exato, Base64 e URL-encoded em
   stdout, stderr, SSE, diffs e artefatos antes da persistência/transmissão.
7. Configurações legadas do `io_bus` com `*_ref` são importadas uma vez para o
   Vault no boot, de forma idempotente; o `SecretStore` permanece como fallback
   de leitura durante a transição.

## Consequências

- O backup do banco carrega o Vault; a chave continua sendo derivada da máquina.
- O LLM nunca vê a credencial; o usuário resolve login/2FA via HITL.
- Toda tentativa de injeção gera auditoria, inclusive as negadas.
- A migration `0002` cria as tabelas e o teste de upgrade cobre a preservação
  dos dados de I/O existentes.
