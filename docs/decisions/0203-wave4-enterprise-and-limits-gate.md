# 0203 — Onda 4 (início): enterprise auth/computer e gate de limites

- **Status:** aceito como entrega parcial
- **Data:** 2026-11-22
- **Contexto:** a extinção de `opendesk-core` depende de concluir as Ondas 2 e 3
  (fila/runner/DAG/telemetria ainda no core, `db` ainda no core). `sandbox`
  depende de `db::settings`, e `nodes`/`workspace_sync`/`skill_*` dependem de
  repositórios ainda não extraídos. Para avançar sem violar fronteiras, foram
  movidos os módulos enterprise autocontidos e o gate de limites foi criado.

## Decisão

1. **`opendesk-enterprise`** com quatro sub-features independentes:
   - `ent-auth`: `mtls` e `update_signing` (verificação estrita de CA/SAN e
     assinatura Ed25519);
   - `ent-computer`: `virtual_display` (Xvfb isolado);
   - `ent-cluster` e `ent-eval`: flags criadas, módulos pendentes da extração de
     `db`.
   O core reexporta `mtls`, `update_signing` e `virtual_display`; o shell não
   mudou de API. `cargo check -p opendesk-enterprise --no-default-features
   --features ent-auth` e `--features ent-computer` passam isoladamente.
2. **Gate de limites** (`scripts/check-rust-limits.py`, executado pelo
   `scripts/gate.sh`):
   - falha para qualquer `.rs` acima de 500 linhas;
   - falha para `.unwrap()`/`.expect(` fora de `#[cfg(test)]`;
   - dívida legada fica em `scripts/rust-limits-baseline.txt` como ratchet:
     arquivo novo ou contagem maior falha; baseline só encolhe.
3. O baseline atual tem 31 entradas (god-files de `core` e `run_export`,
   `adapters/cli`, `config_sync`, etc.). Cada onda deve removê-las, não
   aumentá-las.

## Consequências

- Auth e computer-use já vivem no crate enterprise; cluster/eval e `sandbox`
  continuam no core até `db` migrar.
- O gate impede dívida nova de tamanho/`unwrap`, sem fingir que a dívida antiga
  não existe.
- Testes: 340 shell + 258 core + 27 domain + 6 engine + 3 mcp + 13 recipes +
  15 storage + 4 channels + 6 enterprise (**672**).
- **Pendente da Onda 4:** extinção do core, `sandbox`/`nodes`/`workspace_sync`/
  `skill_*`, shell fino, frontend >500/virtualização/polling duplicado, doctor
  com probes reais e cobertura de 75%.
