# 0213 — Segredos do I/O bus fail-closed com migração única

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** `io_bus` aceitava credenciais inline em `chat_bridge`
  (`token`, `access_token`), `sql_poller`/`sql_sink` (`database_url`) e
  `imap_idle` (`password`). O helper `secret()` priorizava o valor em texto
  plano e só consultava o `SecretStore` como fallback. Qualquer config antiga
  podia, portanto, manter segredos no SQLite, em backups e em logs de erro.

## Decisão

1. Configs de inbound/outbound só aceitam referências: `token_ref`,
   `access_token_ref`, `password_ref`, `database_url_ref`, `secret_ref`.
2. `secrets_guard::validate_no_plaintext_secrets` percorre objetos e arrays e
   rejeita strings não vazias em `token`, `access_token`, `password`, `secret`,
   `api_key`, `database_url` e `private_key`. A validação roda:
   - nos handlers `POST`/`PATCH` de sources e sinks, antes de persistir;
   - nos gatilhos criados por rotinas/receitas;
   - em `inbound::run` e `outbound::execute`, como última barreira.
3. `SecretStore` é a única fonte em runtime. `io_bus::secret` não aceita mais
   valor inline; referência ausente ou chave inexistente falham com 400/404.
4. Migração única no boot (`migrate_legacy_secrets`): cada valor inline vira um
   segredo `legacy-<kind>-<id>-<campo>-<caminho>` no namespace `io_bus`, o JSON
   recebe o `*_ref` e o campo plaintext é removido. Se a migração falhar, a linha
   fica intacta e o runtime a rejeita (fail-closed por source/sink, sem derrubar
   o app).
5. Erros de conexão SQL passam por `redact_connection_url`, removendo
   `user:password@` antes de chegar à API, ao log ou à auditoria.

## Consequências

- Bancos existentes são migrados automaticamente na primeira abertura; não há
  migration de schema (`0001_v2_baseline.sql` permanece intocado).
- Configs inválidas antigas param de executar até serem corrigidas, em vez de
  rodar com credenciais em texto plano.
- Novos testes cobrem validação recursiva, referência ausente, migração
  idempotente e 400 nos endpoints.
- Segredos migrados continuam sujeitos ao `SecretStore` (AES-256-GCM, 0600).
