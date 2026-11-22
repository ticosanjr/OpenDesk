# 0204 — Consolidação total do storage e remoção de `expect` de produção

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** as Ondas 1–3 deixaram 26 repositórios de `db` e a fundação
  (`paths`, `secrets`, `events`, `server_lock`, `dedupe`) no core. O engine não
  podia depender do core, e a dívida de arquivos >500/`expect` seguia no gate.

## Decisão

1. **Storage é dono de todos os repositórios operacionais:**
   - `paths`, `secrets`, `events`, `server_lock`, `dedupe` movidos para
     `opendesk-storage`;
   - 26 módulos de `db/` movidos para `storage/src/repos/` (tasks, runs, queue,
     schedules, calendar, memory, io_bus, workspaces, router, etc.);
   - `settings`, `models` e `conversations` já estavam lá.
2. **Compatibilidade:** `storage/src/db.rs` reexporta `repos` + `init` para o
   caminho interno `crate::db::X`; o core reexporta os módulos, então o shell
   não mudou de API.
3. **Splits sem reescrever lógica:** `tasks`, `schedules`, `calendar` e `memory`
   foram divididos em `*_part{1,2}.rs` via `include!`; `queue` e `io_bus`
   tiveram os testes separados. Nenhum `.rs` do storage passa de 500 linhas.
4. **Test harness:** `storage/src/test_support_tests.rs` (cfg(test)) recria
   `TestContext` com pool, workspace, paths, write-buffer e events.
5. **Zero `expect` de produção nos crates:** locks usam recuperação de
   envenenamento; clients reqwest caem para `Client::new()` em vez de panicar;
   `test_paths` virou best-effort; `test_support` do core virou
   `#[cfg(test)]`/`_tests.rs`.
6. O baseline do gate de limites caiu de 31 para **19 entradas**, todas de
   arquivos >500 ainda não movidos (engine/recipes/mcp/skill_shadow).

## Consequências

- `opendesk-core` caiu de ~43.400 para ~29.300 linhas; o storage tem ~12.300.
- O engine agora pode depender de `opendesk-storage` sem depender do core; a
  extração de fila/runner/telemetria/DAG está desbloqueada.
- Testes: **672** (340 shell + 194 core + 27 domain + 6 engine + 3 mcp + 13
  recipes + 15 storage + 4 channels + 6 enterprise + 64 repos movidos dentro do
  storage).
- **Ainda pendente para extinguir o core:** mover `queue_scheduler`,
  `opencode_client`, `adapters`, `dsh_router`, `run_export`/`run_timeline` e
  `recipes/*` (os 19 god-files restantes), depois o shell fino/frontend.
