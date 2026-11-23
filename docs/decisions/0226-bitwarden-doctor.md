# 0226 — Bitwarden CLI, importação selecionada e doctor ampliado

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** o Vault já tinha importação de `.env`, mas faltava trazer
  credenciais do gerenciador usado no dia a dia sem expor a senha mestre ou a
  sessão do Bitwarden. O `doctor --live` também não cobria Vault, pollers,
  receitas e os conectores novos.

## Decisão

1. O binário `bw` entra no catálogo de ferramentas, com detecção por PATH,
   `OPENDESK_TOOL_BW` ou caminho manual. Nenhum `pkexec`/sudo é usado.
2. `POST /api/vault/import-bitwarden` recebe `BW_SESSION`, `workspace_id` e
   `item_ids` opcionais; a sessão vai apenas no ambiente do processo filho,
   nunca é persistida, logada ou devolvida. Erros do `bw` têm a sessão redigida.
3. A importação cria entradas com `allowed_domains` extraídos das URIs do item,
   `requires_hitl=true` e sem ACL. Entradas `login` sem URI são ignoradas.
4. A UI do Vault oferece importação com sessão e lista opcional de IDs; a lista
   vazia importa todos os itens elegíveis.
5. `doctor --live` passa a verificar:
   - Vault: entradas expiradas e sem ACL de injeção;
   - HTTP Pollers: fontes ativas e com erro;
   - receitas habilitadas: YAML válido;
   - Discord: token via `/users/@me`;
   - Slack: tokens via `auth.test`;
   - WhatsApp QR: sidecar configurado e último status.

## Consequências

- O operador não precisa copiar segredo para o OpenDesk: o Vault recebe o item
  já cifrado com a chave da máquina.
- A sessão do Bitwarden tem vida curta e não aparece em backups/logs.
- O doctor aponta problemas de configuração antes de um run real.
- `live.rs` foi dividido em `live_system.rs` para respeitar o limite de 500
  linhas.
