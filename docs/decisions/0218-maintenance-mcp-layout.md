# 0218 — Manutenção no storage e MCPs sob `src/mcp/`

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** `db/maintenance` morava no shell e chamava `passkey` (também shell)
  para limpar cerimônias WebAuthn. Os MCPs estavam espalhados em cinco arquivos
  no topo de `src-tauri/src` (`mcp_bridge`, `mcp_control`, `mcp_hub`,
  `mcp_sampling`, `mcp_session`), com a pasta `mcp_control/` separada.

## Decisão

1. `db/maintenance.rs` mudou para `opendesk-storage/src/maintenance.rs`.
   A limpeza de cerimônias passou a ser SQL direto em `passkey_ceremonies`
   (`expires_at <= now`), removendo a dependência do módulo `passkey` do shell.
   O shell mantém `db::maintenance` como reexport.
2. Os cinco módulos MCP foram consolidados em `src-tauri/src/mcp/`:
   `bridge`, `control` (com `payload/tools/util`), `hub`, `sampling` e `session`.
   O `lib.rs` reexporta os nomes antigos (`mcp_bridge`, `mcp_control` etc.) para
   preservar todos os callers e testes. O registro puro de sessões streams
   (`mcp::session`) mudou para `opendesk-mcp::session`; payloads de ferramentas
   do control plane continuam no shell por serem específicos da API OpenDesk.
3. `passkey`, `audit` e `sandbox` foram mantidos onde estavam, conforme decisão
   de arquitetura:
   - passkey/audit são concerns de auth/HTTP do shell;
   - sandbox é usado pelo engine (adapters/runtime/CLI) e movê-lo para
     enterprise criaria ciclo de dependência.
4. O `src-tauri/src/db/` continua como fachada de 30 linhas; não há SQL próprio.

## Consequências

- Storage passa a ser dono da manutenção periódica e da retenção de dados.
- MCPs têm um diretório único, sem arquivos soltos no topo do shell.
- Nenhuma API pública mudou; `cargo check --workspace --no-default-features`,
  features `ent-auth`/`ent-computer` e testes continuam verdes.
