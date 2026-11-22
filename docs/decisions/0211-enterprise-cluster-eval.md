# 0211 — Cluster e avaliação em `opendesk-enterprise`

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** nodes, workspace sync e as três camadas de avaliação de skills
  ainda viviam no core.

## Decisão

- `nodes` e `workspace_sync` vão para `opendesk-enterprise::cluster`, atrás de
  `ent-cluster`.
- `skill_eval`, `skill_cases` e `skill_shadow` vão para `opendesk-enterprise::eval`,
  atrás de `ent-eval`; shadow foi dividido em módulo + testes.
- Features `ent-cluster`/`ent-eval` entram no default do crate para o core
  reexportar sem `cfg` próprio; builds mínimos continuam possíveis com
  `--no-default-features --features ...`.
- Doctor passa a verificar WAL/integridade do SQLite, sandbox e display.

## Consequências

- Core só reexporta; nenhum desses módulos depende do shell.
- Path traversal de bundle e lease continuam cobertos por testes do crate.
