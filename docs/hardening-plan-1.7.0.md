# Plano de hardening e refatoração final — OpenDesk 1.7.0+

**TL;DR:** A refatoração V2 até a `1.7.0` extinguiu `opendesk-core` e zerou a baseline de `unwrap()`, mas deixou **8 falhas reais e dívidas estruturais graves** escondidas no código atual: (1) segredos em texto plano ainda aceitos no `io_bus`; (2) extração de ZIP/URL sem travas declaradas contra zip-slip/zip-bomb/SSRF em `studio_ops_install.rs`; (3) chamadas diretas a plugins Tauri no frontend que quebram em modo PWA/browser; (4) splits artificiais via `include!` (`*_part1/2.rs`) no `opendesk-storage`; (5) duplicação de `recurrence.rs` no mesmo crate e envio Telegram/SMTP/Signal replicado em 3–4 camadas; (6) `passkey`, `audit`, `db/maintenance` e `sandbox` fora de seus crates definitivos; (7) polling em `chat.rs::wait_for_turn` apesar do SSE global; e (8) limite de 500 linhas desligado no shell (`--no-line-limit`) + docs obsoletos. Abaixo está a especificação atômica em 8 microspecs (`P0`–`P2`) pronta para execução sequencial pelo DeepSeek v4 no OpenCode.

---

## 1. Objetivo Final e Escopo

### Objetivo Final

Levar o OpenDesk `1.7.0` ao estado de engenharia **fail-closed, sem duplicação de transporte/domínio, sem hacks de `include!`, seguro em modo PWA e com limite real de 500 linhas aplicado também ao shell (`src-tauri/src/`)**, mantendo 100% dos 676+ testes Rust, 256 testes Vitest e suíte Playwright verdes.

### Fora de Escopo (NÃO Alterar)

* **Nenhuma funcionalidade nova de produto** (novos canais, novas páginas de UI ou alterações visuais que quebrem goldens do Playwright).

* **Contratos JSON da API HTTP/WebSocket consumidos pelo frontend** (ex.: payload de `/api/attention`, `/api/tasks`, `/api/chat`, `/api/schedules`) — preservar compatibilidade para não quebrar os 55+ specs E2E em `tests/e2e/`.

* **Schema `0001_v2_baseline.sql` (61 tabelas STRICT)** — não alterar colunas nem criar migration `0002`.

* **Conteúdo dos Starter Packs** em `starter-packs/` e scripts de publicação (`scripts/release.sh`).

* **Builds nativos de macOS/Windows** fora das correções de `#[cfg(unix)]`.

---

## 2. Mapa de Arquivos

| Arquivo | Ação | Motivo |
| --- | --- | --- |
| `src-tauri/src/io_bus/inbound/chat_bridge.rs` | Modificar | Remover `token`/`access_token` em texto plano; exigir `*_ref` via `SecretStore`. |
| `src-tauri/src/io_bus/outbound/messaging.rs` | Modificar | Remover `token` em texto plano e delegar Telegram/SMTP/Signal para `opendesk-channels`. |
| `src-tauri/src/io_bus/inbound/sql_poller.rs` | Modificar | Rejeitar `database_url` em texto plano; exigir `database_url_ref` e mascarar erros SQL. |
| `src-tauri/src/io_bus/outbound/sql_sink.rs` | Modificar | Rejeitar `database_url` em texto plano; exigir `database_url_ref` e mascarar erros SQL. |
| `src-tauri/src/io_bus/sql.rs` | Modificar | Adicionar sanitização de credenciais em mensagens de erro de conexão. |
| `src-tauri/src/api/io_bus.rs` | Modificar | Validar fail-closed `validate_no_plaintext_secrets` na criação/edição de sources e sinks. |
| `src-tauri/src/io_bus/chat.rs` | Modificar | Remover `send_telegram` duplicado (usar `opendesk-channels::telegram`). |
| `src-tauri/src/api/studio_ops_install.rs` | Modificar | Blindar `download_url`/`collect_zip`/`collect_local_dir` (SSRF, zip-slip, zip-bomb, symlinks). |
| `src-tauri/src/api/updates.rs`, `src-tauri/src/updates.rs`, `src-tauri/src/io_bus/tools.rs` | Modificar | Garantir `#[cfg(unix)]` em todos os usos de `PermissionsExt` e fallback explícito não-Unix. |
| `src/lib/runtime-env.ts` | Criar | Adaptador único `isTauriRuntime()`, `openExternalUrl()`, `openLocalPath()` (Tauri vs PWA). |
| `src/components/integrations/BrowserCard.tsx`, `src/components/settings/AboutCard.tsx`, `src/pages/CapturePage.tsx`, `src/lib/open-path.ts` | Modificar | Substituir chamadas diretas a `@tauri-apps/plugin-opener`/`invoke` e `isTauri()` local. |
| `src-tauri/crates/opendesk-storage/src/repos/{calendar,memory,schedules,tasks}_part{1,2}.rs` | Remover | Eliminar splits artificiais com `include!` e substituir por submódulos Rust reais (`<modulo>/mod.rs`, `queries.rs`, `mutations.rs`). |
| `src-tauri/crates/opendesk-storage/src/repos/{chat,chat_turns,interactions,task_messages}.rs` | Modificar / Remover | Eliminar duplicação com `repos/conversations/*` (migrar callers ou reduzir a re-export puro de 1 linha se exigido). |
| `src-tauri/crates/opendesk-engine/src/queue_scheduler/recurrence.rs` | Remover | Duplicado de `opendesk-engine/src/scheduler/recurrence.rs`. |
| `src-tauri/crates/opendesk-recipes/src/cron.rs` | Modificar / Remover | Delegar integralmente a `opendesk_engine::scheduler::recurrence`. |
| `src-tauri/src/integrations/delivery/{email,signal_channel,telegram_channel}.rs` e `src-tauri/src/recipes/hitl/{email,signal,telegram}.rs` | Modificar | Eliminar lógica de transporte duplicada; delegar envio para `opendesk-channels`. |
| `src-tauri/src/passkey.rs`, `src-tauri/src/audit.rs` | Remover (Mover) | Mover para `crates/opendesk-enterprise/src/auth/{passkey,audit}.rs` atrás de `ent-auth`. |
| `src-tauri/src/db/maintenance.rs` | Remover (Mover) | Mover para `crates/opendesk-storage/src/maintenance.rs` após desacoplar de `passkey`. |
| `src-tauri/crates/opendesk-engine/src/sandbox.rs` | Remover (Mover) | Mover para `crates/opendesk-enterprise/src/computer/sandbox.rs` (`ent-computer`) + trait port em `opendesk-domain`. |
| `src-tauri/src/mcp_{bridge,control,hub,sampling,session}.rs` | Remover (Mover) | Consolidar em `src-tauri/src/mcp/` (ou mover protocolo puro para `opendesk-mcp`). |
| `src-tauri/src/chat.rs` | Modificar | Trocar polling em `wait_for_turn` por inscrição em eventos SSE (`session.idle` / `session.execution.*`). |
| `scripts/check-rust-limits.py`, `scripts/gate.sh`, `scripts/check-changed.sh` | Modificar | Remover `--no-line-limit` do shell e proibir `include!(` em código de produção Rust. |
| `docs/implementation-plan.md` | Remover (Mover) | Mover para `docs/archive/v1/implementation-plan.md` (substituído por `ARCHITECTURE.md` e `SPEC.md`). |
| `docs/security/update-signing.md` | Modificar | Corrigir caminho da chave pública para `src-tauri/crates/opendesk-enterprise/src/update-signing-public-key.txt`. |

---

## 3. Contratos, Schemas e Invariantes

### 3.1 Contrato Fail-Closed de Segredos no `io_bus` (`P0.1`)

```rust
/// Chaves proibidas em qualquer JSON de configuração de InboundSource ou OutboundSink.
pub const FORBIDDEN_PLAINTEXT_SECRET_KEYS: &[&str] = &[
    "token",
    "access_token",
    "password",
    "secret",
    "api_key",
    "database_url",
    "private_key",
];

/// Percorre recursivamente `serde_json::Value`.
/// Retorna `AppError::Validation` (HTTP 400) se encontrar qualquer chave proibida
/// com string não vazia. Nunca loga o valor encontrado.
pub fn validate_no_plaintext_secrets(value: &serde_json::Value) -> AppResult<()>;

/// Remove credenciais (`user:pass@`) de qualquer URL antes de retornar ou persistir erros.
pub fn redact_connection_url(raw: &str) -> String;
```

* **Invariante:** Nenhum `InboundSource` ou `OutboundSink` pode ser salvo nem executado com segredo inline; o runtime resolve exclusivamente `*_ref` via `SecretStore`. Se `*_ref` estiver ausente ou não existir no cofre, falhar explicitamente com `AppError::Validation` / `AppError::NotFound` (nunca degradar silenciosamente).

### 3.2 Contrato de Segurança de Download e ZIP (`P0.2`)

```rust
pub struct SafeArchiveLimits {
    pub max_download_bytes: u64,      // 25 * 1024 * 1024 (25 MiB)
    pub max_entries: usize,           // 250
    pub max_uncompressed_bytes: u64,  // 50 * 1024 * 1024 (50 MiB)
    pub max_compression_ratio: u64,   // 50 (bloqueia zip-bomb)
}

/// Valida URL externa antes de efetuar GET:
/// - Permite apenas esquema `https` (e `http://127.0.0.1` somente em `#[cfg(test)]`).
/// - Rejeita IPs loopback, RFC1918 (10/8, 172.16/12, 192.168/16), link-local (169.254/16, fe80::/10) e IPv6 ULA.
pub async fn validate_external_url(url: &str) -> AppResult<reqwest::Url>;
```

* **Invariante ZIP:** Usar `entry.enclosed_name()` obrigatório; rejeitar entradas `None`, symlinks (modo Unix `0o120000`), contagens `> max_entries` ou leitura acumulada `> max_uncompressed_bytes` usando `Read::take(remaining + 1)` antes de alocar em memória.

### 3.3 Contrato de Runtime Frontend (`P0.3` — `src/lib/runtime-env.ts`)

```typescript
export function isTauriRuntime(): boolean;
export async function openExternalUrl(url: string): Promise<void>;
export async function openLocalPath(path: string): Promise<{ ok: boolean; fallback?: string }>;
```

* **Invariante:** Nenhum componente React em `src/components/` ou `src/pages/` pode importar `@tauri-apps/plugin-opener` ou testar `window.__TAURI__` manualmente; todos consomem `src/lib/runtime-env.ts`. Em modo browser/PWA, `openLocalPath` não lança exceção não tratada — copia o caminho ou exibe toast orientando uso local.

### 3.4 Contrato de Porta de Sandbox (`P1.3` — `opendesk-domain`)

```rust
#[async_trait::async_trait]
pub trait SandboxPort: Send + Sync {
    async fn wrap_command(&self, spec: SandboxCommandSpec) -> Result<WrappedCommand, DomainError>;
    async fn health_probe(&self) -> SandboxHealth;
}
```

* **Invariante:** `opendesk-engine` chama `Arc<dyn SandboxPort>` (ou implementação no-op controlada por política quando `ent-computer` estiver desligado com `--no-default-features`). Quando a política exigir sandbox e nenhum backend (`bwrap`/`docker`/`podman`) estiver disponível, **falhar fechado** (`DomainError::SandboxUnavailable`) — nunca executar no host sem sandbox.

### 3.5 Invariante de Limites de Código (`P2.1`)

* Todo arquivo `.rs` em `src-tauri/crates/` e `src-tauri/src/` (exceto testes) deve ter **`<= 500` linhas**, **zero `.unwrap()` / `.expect(**` e **zero `include!(**`.
* `scripts/rust-limits-baseline.txt` e `scripts/shell-limits-baseline.txt` devem permanecer **100% vazios (0 bytes / 0 entradas)**.

---

## 4. Fases Atômicas de Implementação (Microspecs)

### Fase 1 (`P0.1`) — Blindagem Fail-Closed de Segredos no `io_bus`

* **Arquivos da fase:**
* Ler e modificar: `src-tauri/src/io_bus/inbound/chat_bridge.rs`, `src-tauri/src/io_bus/outbound/messaging.rs`, `src-tauri/src/io_bus/inbound/sql_poller.rs`, `src-tauri/src/io_bus/outbound/sql_sink.rs`, `src-tauri/src/io_bus/sql.rs`, `src-tauri/src/api/io_bus.rs`, `src-tauri/src/api/tests/io_bus.rs`.

* Não tocar: `0001_v2_baseline.sql`, frontend `src/`.

* **Passos determinísticos:**
1. Criar função `validate_no_plaintext_secrets` e `redact_connection_url` em `src-tauri/src/io_bus/mod.rs` (ou submódulo `src-tauri/src/io_bus/secrets_guard.rs`).
2. Em `src-tauri/src/api/io_bus.rs`, invocar `validate_no_plaintext_secrets` em todos os handlers de `POST` e `PATCH`/`PUT` de inbound sources e outbound sinks antes de persistir no banco.
3. Remover os campos plaintext `token` (`TelegramConfig`, `messaging::Config`), `access_token` (`MatrixConfig`) e `database_url` (`PollerConfig`, `sql_sink::Config`); manter apenas `token_ref`, `access_token_ref` e `database_url_ref`.
4. Em `src-tauri/src/io_bus/sql.rs`, interceptar erros de `AnyPoolOptions::connect` e aplicar `redact_connection_url` antes de propagar `AppError`.
5. Atualizar os testes unitários em `sql_poller.rs`, `sql_sink.rs`, `messaging.rs` e `api/tests/io_bus.rs` para gravar segredos no `SecretStore` de teste e adicionar 3 testes explícitos provando que payloads com `token`, `access_token` ou `database_url` em texto plano retornam `400 Bad Request`.

* **Refatoração/Simplificação/Limpeza acoplada:** Remover funções auxiliares duplicadas `fn secret(...)` entre `chat_bridge.rs` e `messaging.rs`, substituindo por uma única helper `resolve_secret_ref(&SecretStore, Option<&str>, &str) -> AppResult<String>`.

* **Edge cases obrigatórios:**
* Chaves proibidas aninhadas em sub-objetos JSON ou arrays devem ser detectadas recursivamente.
* Strings vazias (`""`) ou `null` nos campos legados (se vierem de formulários antigos) podem ser ignoradas apenas se `*_ref` estiver presente; qualquer string não vazia em campo proibido **deve** falhar fechado (400).
* `sqlite::memory:` em testes deve passar via `secret_ref` registrado no `SecretStore` do `TestContext`.

* **Validação:**
* `cd src-tauri && cargo test io_bus` -> **PASS** (100% dos testes de `io_bus` + novos testes de rejeição 400).
* `rg -n 'pub (token|access_token|database_url):' src-tauri/src/io_bus/` -> **PASS** (saída vazia).

---

### Fase 2 (`P0.2`) — Blindagem de `studio_ops_install.rs` e Guards `#[cfg(unix)]`

* **Arquivos da fase:**
* Ler e modificar: `src-tauri/src/api/studio_ops_install.rs`, `src-tauri/src/api/tests/studio.rs`, `src-tauri/src/api/updates.rs`, `src-tauri/src/updates.rs`, `src-tauri/src/io_bus/tools.rs`.

* **Passos determinísticos:**
1. Em `src-tauri/src/api/studio_ops_install.rs`, implementar `validate_external_url` (bloqueio de esquemas != `https` e IPs privados/loopback/link-local resolvidos via `tokio::net::lookup_host`).
2. Em `download_url_auth`, ler o corpo em streaming com teto rígido de `25 MiB` (abortar imediatamente ao exceder, sem carregar em `Vec<u8>` ilimitado) e configurar `reqwest::redirect::Policy` que revalida cada redirect contra `validate_external_url`.
3. Em `collect_zip`, impor `SafeArchiveLimits` (`max_entries = 250`, `max_uncompressed_bytes = 50 MiB`, `max_compression_ratio = 50`), rejeitar `entry.enclosed_name().is_none()` e entradas symlink (`unix_mode() & 0o170000 == 0o120000`), lendo cada entrada via `.take(remaining + 1)`.
4. Em `collect_local_dir`, canonicalizar o root e rejeitar symlinks que apontem para fora do root.
5. Auditar todos os `use std::os::unix::fs::PermissionsExt;` em `api/updates.rs`, `updates.rs` e `io_bus/tools.rs` garantindo `#[cfg(unix)]` explícito em imports e blocos de permissão.
6. Escrever testes unitários em `studio_ops_install.rs` cobrindo: URL `[http://169.254.169.254/](http://169.254.169.254/)...` rejeitada, ZIP com `../escape.md` rejeitado, ZIP excedendo `max_entries` ou `max_uncompressed_bytes` rejeitado.

* **Refatoração/Simplificação/Limpeza acoplada:** Se `studio_ops_install.rs` passar de 450 linhas, extrair validação de archive/URL para `src-tauri/src/api/studio_install_safety.rs`.

* **Edge cases obrigatórios:**
* Redirect HTTP 302 de domínio público para `[http://127.0.0.1](http://127.0.0.1)` deve ser bloqueado.
* ZIP válido com 0 bytes de conteúdo em um arquivo não pode causar divisão por zero no cálculo de `max_compression_ratio`.

* **Validação:**
* `cd src-tauri && cargo test studio` -> **PASS**.
* `cd src-tauri && cargo clippy --workspace --all-targets -- -D warnings` -> **PASS**.

---

### Fase 3 (`P0.3`) — Adaptador Único de Runtime Frontend (`Tauri` vs `PWA/Browser`)

* **Arquivos da fase:**
* Criar: `src/lib/runtime-env.ts`, `src/lib/runtime-env.test.ts`.
* Ler e modificar: `src/components/integrations/BrowserCard.tsx`, `src/components/settings/AboutCard.tsx`, `src/pages/CapturePage.tsx`, `src/lib/open-path.ts`, `src/lib/open-path.test.ts`.

* **Passos determinísticos:**
1. Criar `src/lib/runtime-env.ts` com `isTauriRuntime()`, `openExternalUrl(url)` e `openLocalPath(path)`:
* Quando `isTauriRuntime()` for `true`, delegar para `@tauri-apps/plugin-opener` (`openUrl`) e `invoke("open_workspace_path", ...)` (preservando a allowlist Rust documentada em `src/lib/open-path.ts`).
* Quando `isTauriRuntime()` for `false`, `openExternalUrl` usa `window.open(url, "_blank", "noopener,noreferrer")` (validando esquema `https:`/`http:`) e `openLocalPath` retorna `{ ok: false, fallback: path }` sem lançar exceção.

2. Atualizar `BrowserCard.tsx`, `AboutCard.tsx` e `CapturePage.tsx` para remover imports diretos de `@tauri-apps/plugin-opener` e `function isTauri()` local, usando `src/lib/runtime-env.ts` e exibindo toast informativo quando `!result.ok` em modo PWA.

3. Criar `src/lib/runtime-env.test.ts` testando ambos os modos (Tauri mockado e browser puro) e bloqueio de `javascript:` em `openExternalUrl`.

* **Refatoração/Simplificação/Limpeza acoplada:** Checar com `rg "@tauri-apps/" src/` se restou algum outro componente em `src/components/` ou `src/pages/` sem guarda de modo browser e rotear pelo adaptador.

* **Edge cases obrigatórios:**
* `openExternalUrl("javascript:alert(1)")` deve lançar erro de validação (fail-closed), nunca chamar `window.open` nem `openUrl`.
* Em testes Vitest (jsdom), `isTauriRuntime()` deve retornar `false` por padrão sem quebrar testes existentes.

* **Validação:**
* `npm run check && npm run lint && npm run test:ui` -> **PASS**.
* `rg "@tauri-apps/plugin-opener" src/components src/pages` -> **PASS** (saída vazia).

---

### Fase 4 (`P1.1`) — Remover `include!` no Storage, Duplicatas de `recurrence` e Shims de Conversas

* **Arquivos da fase:**
* Ler, refatorar e remover:
* `src-tauri/crates/opendesk-storage/src/repos/{calendar,memory,schedules,tasks}.rs` e `*_part{1,2}.rs`.
* `src-tauri/crates/opendesk-storage/src/repos/{chat,chat_turns,interactions,task_messages}.rs` e `conversations/*`.
* `src-tauri/crates/opendesk-engine/src/queue_scheduler/recurrence.rs`, `src-tauri/crates/opendesk-engine/src/scheduler/recurrence.rs`, `src-tauri/crates/opendesk-recipes/src/cron.rs`.

* Ler e atualizar imports se necessário: `src-tauri/crates/opendesk-storage/src/repos/mod.rs`, `src-tauri/crates/opendesk-engine/src/queue_scheduler/mod.rs`, `src-tauri/crates/opendesk-recipes/src/lib.rs`.

* **Passos determinísticos:**
1. Converter cada grupo com `include!` em diretório de módulo Rust real (com `mod queries; mod mutations; pub use ...;`), mantendo **exatamente** a mesma API pública (`opendesk_storage::repos::{tasks,schedules,calendar,memory}::*`) para não quebrar callers:
* `repos/tasks/{mod,queries,mutations,workflow}.rs` + `tests.rs`
* `repos/schedules/{mod,queries,mutations}.rs` + `tests.rs`
* `repos/calendar/{mod,events,sources}.rs` + `tests.rs`
* `repos/memory/{mod,namespaces,notes}.rs` + `tests.rs`

2. Deletar os 8 arquivos `*_part1.rs` e `*_part2.rs`.

3. Comparar `repos/{chat,chat_turns,interactions,task_messages}.rs` com `repos/conversations/{threads,messages,turns,task,interactions}.rs`: eliminar qualquer SQL ou struct duplicada; manter a lógica canônica apenas dentro de `repos/conversations/` e expor aliases finos em `repos/mod.rs` (`pub use conversations::threads as chat;`, etc.), deletando os 4 arquivos duplicados se não tiverem código exclusivo.

4. Comparar `opendesk-engine/src/queue_scheduler/recurrence.rs` com `opendesk-engine/src/scheduler/recurrence.rs`: fundir qualquer função exclusiva em `scheduler/recurrence.rs`, deletar `queue_scheduler/recurrence.rs` e re-exportar via `pub use crate::scheduler::recurrence;` em `queue_scheduler/mod.rs`.

5. Auditar `opendesk-recipes/src/cron.rs`: remover qualquer reimplementação de parsing/próxima execução cron e delegar 100% para `opendesk_engine::scheduler::recurrence`.

* **Refatoração/Simplificação/Limpeza acoplada:** Garantir que cada novo submódulo em `opendesk-storage/src/repos/` tenha `<= 500` linhas reais sem truques de pré-processamento.

* **Edge cases obrigatórios:**
* Nenhum símbolo público em `opendesk_storage::repos::*` pode sumir ou mudar assinatura nesta fase.
* Todos os testes de `calendar_tests`, `memory_tests`, `schedules_tests` e `tasks_tests` devem continuar executando sob `cargo test -p opendesk-storage`.

* **Validação:**
* `rg -n 'include!\(' src-tauri/crates/` -> **PASS** (saída vazia).
* `test ! -f src-tauri/crates/opendesk-engine/src/queue_scheduler/recurrence.rs` -> **PASS**.
* `cd src-tauri && cargo test -p opendesk-storage -p opendesk-engine -p opendesk-recipes` -> **PASS**.

---

### Fase 5 (`P1.2`) — Unificar Transporte de Canais (Eliminar 3ª Cópia em `io_bus`, `delivery` e `recipes/hitl`)

* **Arquivos da fase:**
* Ler e modificar: `src-tauri/crates/opendesk-channels/src/{telegram,signal,email}.rs` (e `lib.rs`), `src-tauri/src/io_bus/chat.rs`, `src-tauri/src/io_bus/outbound/messaging.rs`, `src-tauri/src/io_bus/inbound/chat_bridge.rs`, `src-tauri/src/integrations/delivery/{email,signal_channel,telegram_channel}.rs`, `src-tauri/src/recipes/hitl/{email,signal,telegram}.rs`.

* **Passos determinísticos:**
1. Verificar quais primitivas de envio existem hoje em `opendesk-channels` (`telegram::client`, `signal::send`, `email`) e expor assinaturas limpas e reutilizáveis para envio de texto + anexos (ex.: `telegram::send_message`, `telegram::send_document`, `email::send_email`, `signal::send_message`).

2. Em `src-tauri/src/io_bus/chat.rs`, remover a implementação própria de `send_telegram` (delegar ou remover e atualizar callers em `io_bus/outbound/messaging.rs` e `io_bus/inbound/chat_bridge.rs` para chamar `opendesk_channels::telegram::*`).

3. Em `src-tauri/src/io_bus/outbound/messaging.rs`, substituir chamadas diretas a `reqwest` (Telegram) / `lettre` (SMTP) / CLI (Signal) pelas funções de `opendesk-channels`.

4. Em `src-tauri/src/integrations/delivery/{email,signal_channel,telegram_channel}.rs` e `src-tauri/src/recipes/hitl/{email,signal,telegram}.rs`, remover código de transporte HTTP/SMTP/CLI duplicado, mantendo apenas montagem do conteúdo da mensagem/HITL e delegando o envio + política `NotificationRouter` a `opendesk-channels`.

* **Refatoração/Simplificação/Limpeza acoplada:** Checar `src-tauri/Cargo.toml` e remover dependências diretas de transporte de canal no crate raiz se só forem usadas dentro de `opendesk-channels`.

* **Edge cases obrigatórios:**
* Compilação com `--no-default-features` em `opendesk-channels` deve continuar passando (usar `#[cfg(feature = "ch-...")]` corretamente).
* `recipes/hitl` deve continuar enviando botões/instruções de aprovação no Telegram e preservando `Message-ID`/`In-Reply-To` no Email.

* **Validação:**
* `cd src-tauri && cargo check -p opendesk-channels --no-default-features && cargo test -p opendesk-channels --all-features` -> **PASS**.
* `cd src-tauri && cargo test delivery && cargo test hitl && cargo test io_bus` -> **PASS**.

---

### Fase 6 (`P1.3`) — Realocar Domínio Enterprise (`passkey`, `audit`, `sandbox`), `db/maintenance` e Organizar `mcp/`

* **Arquivos da fase:**
* Mover / Criar / Remover:
* `src-tauri/src/passkey.rs` -> `src-tauri/crates/opendesk-enterprise/src/auth/passkey.rs` (`ent-auth`).
* `src-tauri/src/audit.rs` -> `src-tauri/crates/opendesk-enterprise/src/auth/audit.rs` (`ent-auth`).
* `src-tauri/src/db/maintenance.rs` -> `src-tauri/crates/opendesk-storage/src/maintenance.rs`.
* `src-tauri/crates/opendesk-engine/src/sandbox.rs` -> `src-tauri/crates/opendesk-enterprise/src/computer/sandbox.rs` (`ent-computer`) + trait `SandboxPort` em `src-tauri/crates/opendesk-domain/src/ports.rs`.
* `src-tauri/src/mcp_{bridge,control,hub,sampling,session}.rs` -> agrupar sob `src-tauri/src/mcp/` (e mover utilitários puros sem `AppState` para `opendesk-mcp`).

* Atualizar imports em: `src-tauri/src/lib.rs`, `src-tauri/src/db/mod.rs`, `src-tauri/src/api/{audit,passkey,sandbox,mcp,mcp_hub}.rs`, `src-tauri/crates/opendesk-enterprise/Cargo.toml`, `src-tauri/crates/opendesk-engine/Cargo.toml`.

* **Passos determinísticos:**
1. Mover `passkey.rs` e `audit.rs` para `opendesk-enterprise/src/auth/` atrás da feature `ent-auth`; re-exportar em `opendesk_enterprise::auth`.
2. Desacoplar `db/maintenance.rs` de `passkey`: passar a limpeza de desafios WebAuthn expirados como query SQL direta sobre a tabela de passkeys (que já pertence ao `opendesk-storage` em `0001_v2_baseline.sql`) e mover o arquivo para `opendesk-storage/src/maintenance.rs`. Deletar `src-tauri/src/db/maintenance.rs`.
3. Definir `SandboxPort` em `opendesk-domain/src/ports.rs`; mover `sandbox.rs` de `opendesk-engine` para `opendesk-enterprise/src/computer/sandbox.rs` (`ent-computer`) implementando `SandboxPort`; injetar `Arc<dyn SandboxPort>` nos pontos do engine/shell que usam sandbox (com política fail-closed quando sandbox for obrigatório e indisponível).
4. Consolidar `mcp_bridge.rs`, `mcp_control.rs`, `mcp_hub.rs`, `mcp_sampling.rs` e `mcp_session.rs` em `src-tauri/src/mcp/mod.rs` (unificando com a pasta `src-tauri/src/mcp_control/` já existente).

* **Refatoração/Simplificação/Limpeza acoplada:** Deletar `src-tauri/src/db/` caso `db/mod.rs` seja apenas fachada redundante (ou mantê-lo com < 30 linhas de re-export limpo de `opendesk_storage::repos` se muitos arquivos de `api/` usarem `crate::db::*`).

* **Edge cases obrigatórios:**
* `cargo check -p opendesk-enterprise --no-default-features --features ent-auth` e `--features ent-computer` devem compilar isoladamente sem erro.
* `cargo check --workspace --no-default-features` deve compilar sem falhas.

* **Validação:**
* `cd src-tauri && cargo check --workspace --no-default-features` -> **PASS**.
* `cd src-tauri && cargo check -p opendesk-enterprise --no-default-features --features ent-auth && cargo check -p opendesk-enterprise --no-default-features --features ent-computer` -> **PASS**.
* `cd src-tauri && cargo test --workspace --all-features` -> **PASS**.

---

### Fase 7 (`P1.4`) — `chat.rs::wait_for_turn` Event-Driven via SSE

* **Arquivos da fase:**
* Ler e modificar: `src-tauri/src/chat.rs`, `src-tauri/crates/opendesk-engine/src/opencode_client/session_runner/` (para verificar o canal de broadcast SSE existente).

* **Passos determinísticos:**
1. Inspecionar como o assinante SSE global do `opencode_client` publica `session.idle`, `session.execution.succeeded` e `session.execution.failed` (`docs/protocol/opencode-v2.md`).

2. Em `src-tauri/src/chat.rs::wait_for_turn`, assinar o canal de eventos da sessão **antes** de disparar o prompt assíncrono (evitando race condition de resposta ultrarrápida).

3. Aguardar via `tokio::select!`:
* Evento SSE terminal (`session.idle` / `session.execution.succeeded` / `session.execution.failed`) para `session_id` -> executar 1 fetch de confirmação em `/api/session/{id}/message` e validar `turn_has_text` (mantendo a proteção documentada contra mensagens intermediárias apenas de tool call).
* `CancellationToken::cancelled()` -> abortar imediatamente e interromper sessão no OpenCode.
* Fallback de segurança (intervalo de 10s caso o SSE desconecte) + timeout global configurável -> se estourar sem resposta textual, retornar erro explícito (`AppError::Timeout`), nunca retornar a resposta do turno anterior.

4. Manter e ampliar os testes existentes em `chat.rs` (`waits_for_a_new_turn_instead_of_returning_the_previous_answer` e `does_not_stop_on_a_tool_only_message`) adicionando teste de entrega imediata (< 100 ms) quando evento SSE `session.idle` chega.

* **Refatoração/Simplificação/Limpeza acoplada:** Se `src-tauri/src/chat.rs` ultrapassar 450 linhas, separar parsing de mensagens/usage em `src-tauri/src/chat_parse.rs`.

* **Edge cases obrigatórios:**
* Evento `session.idle` de **outra** sessão não pode acordar o turno atual.
* `RecvError::Lagged` no broadcast deve disparar 1 fetch imediato de reconciliação em vez de sair com erro falso.

* **Validação:**
* `cd src-tauri && cargo test chat::tests` -> **PASS**.

---

### Fase 8 (`P2.1 + P2.2`) — Gate de 500 Linhas no Shell, Bloqueio de `include!` e Limpeza de Docs/CSS

* **Arquivos da fase:**
* Ler e modificar: `scripts/check-rust-limits.py`, `scripts/gate.sh`, `scripts/check-changed.sh`, `scripts/shell-limits-baseline.txt`, `docs/security/update-signing.md`, `src/styles.css`.

* Mover: `docs/implementation-plan.md` -> `docs/archive/v1/implementation-plan.md`.

* Fatiar (somente se acusados pelo script): quaisquer arquivos > 500 linhas em `src-tauri/src/` (candidatos prováveis: `starter_packs.rs`, `updates.rs`, `api/mod.rs`, `chat.rs`).

* **Passos determinísticos:**
1. Em `scripts/check-rust-limits.py`, adicionar regra que **falha** se qualquer arquivo `.rs` fora de `#[cfg(test)]` contiver `include!(`.

2. Em `scripts/gate.sh` e `scripts/check-changed.sh`, remover a flag `--no-line-limit` da invocação de `check-rust-limits.py` sobre `src-tauri/src`.

3. Rodar `python3 scripts/check-rust-limits.py --root src-tauri/src` e fatiar todos os arquivos que ultrapassarem 500 linhas em submódulos reais por responsabilidade (sem adicionar nenhuma entrada em `scripts/shell-limits-baseline.txt`).

4. Mover `docs/implementation-plan.md` para `docs/archive/v1/implementation-plan.md` e remover links quebrados no `README.md` / `ARCHITECTURE.md` / `AGENTS.md`.

5. Atualizar `docs/security/update-signing.md` trocando `src-tauri/update-signing-public-key.txt` por `src-tauri/crates/opendesk-enterprise/src/update-signing-public-key.txt`, verificando que `scripts/release.sh` e `opendesk-enterprise/src/auth/update_signing.rs` apontam para o mesmo caminho.

6. Auditar com `rg` os seletores legados em `src/styles.css` (`.text-xs`, `.text-\[11px\]`, `.text-\[12px\]`): remover apenas os seletores sem nenhuma ocorrência em `src/` (nunca alterar tamanho computado de classes ainda usadas para não quebrar goldens visuais).

* **Edge cases obrigatórios:**
* Não alterar nada que modifique layout/tipografia renderizada (os goldens visuais em `playwright.visual.config.ts` devem continuar 100% verdes).
* `scripts/rust-limits-baseline.txt` e `scripts/shell-limits-baseline.txt` devem terminar vazios.

* **Validação:**
* `python3 scripts/check-rust-limits.py --root src-tauri/crates && python3 scripts/check-rust-limits.py --root src-tauri/src && python3 scripts/check-frontend-limits.py` -> **PASS**.
* `./scripts/gate-fast.sh --rust` -> **PASS**.
* `./scripts/gate.sh` -> **PASS** (100% verde incluindo cobertura, clippy, Vitest, Playwright e limites).

---

## 5. Prompt de Execução para o OpenCode

```markdown
Você vai executar a especificação de hardening, limpeza e refatoração final do OpenDesk v1.7.0+ (Fases 1 a 8).

## Regras Obrigatórias de Execução
1. **Uma fase por vez:** Execute rigorosamente na ordem (Fase 1 -> Fase 2 -> ... -> Fase 8). NÃO inicie a próxima fase enquanto TODOS os comandos de validação da fase atual não retornarem PASS.
2. **Leitura dirigida:** Antes de editar qualquer arquivo de uma fase, leia APENAS os arquivos listados em "Arquivos da fase" e use `rg` pontual para localizar callers/imports impactados.
3. **Fail-closed absoluto:**
   - Nunca silencie erros, nunca use `.unwrap()`/`.expect()` fora de `#[cfg(test)]` e nunca adicione `#[allow(...)]` para esconder warnings do clippy.
   - Nunca use `include!(` para dividir código Rust.
   - Nunca adicione entradas em `scripts/rust-limits-baseline.txt` ou `scripts/shell-limits-baseline.txt`.
4. **Preservação de contratos e testes:**
   - Não altere o schema `0001_v2_baseline.sql` nem os payloads JSON dos endpoints HTTP/WS consumidos pelo frontend.
   - Nunca remova, comente ou enfraqueça testes existentes (Rust, Vitest ou Playwright). Atualize-os apenas quando a assinatura interna mudar conforme a spec.
   - Não altere arquivos fora do escopo da fase atual.
5. **Reporte objetivo ao fim de cada fase:** Liste (a) arquivos criados/modificados/removidos, (b) resultado exato dos comandos de validação executados, (c) confirmação de zero regressões antes de prosseguir.

Comece agora pela **Fase 1 (P0.1 — Blindagem Fail-Closed de Segredos no `io_bus`)**. Leia os arquivos da Fase 1 primeiro.
```
