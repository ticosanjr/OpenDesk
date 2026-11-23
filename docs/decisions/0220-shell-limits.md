# 0220 — Limite de 500 linhas no shell e `include!` proibido em produção

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** o gate aplicava 500 linhas apenas em `src-tauri/crates`; o shell
  tinha 31 arquivos de produção acima do limite (o maior com 1756 linhas) e o
  teste de limites era executado com `--no-line-limit`. A dívida ficava invisível.

## Decisão

1. `scripts/check-rust-limits.py` passa a contar linhas também em
   `src-tauri/src`, ignorando apenas arquivos de teste. A flag
   `--no-line-limit` deixou de ser usada no gate e no `check:changed`.
2. `include!(` em código de produção (fora de `#[cfg(test)]`) é uma violação.
   `include!` de teste continua permitido.
3. Os 31 arquivos foram divididos mecanicamente em módulos por concern, com
   `pub use`/reexports preservando caminhos e assinaturas. Nenhuma rota, JSON,
   SQL, prompt, output ou DOM mudou.
4. `docs/implementation-plan.md` foi arquivado em
   `docs/archive/v1/implementation-plan.md`; `docs/security/update-signing.md`,
   `scripts/release.sh` e `auth/update_signing.rs` apontam para a chave pública
   real em `crates/opendesk-enterprise/src/update-signing-public-key.txt`.
5. Seletores CSS sem uso (`type-*`, `surface-*`, `field-base`, etc.) foram
   removidos (`styles.css` 495 → 386 linhas). Nenhuma classe ainda usada foi
   alterada, preservando os goldens.

## Consequências

- `python3 scripts/check-rust-limits.py` e
  `python3 scripts/check-rust-limits.py --root src-tauri/src` passam com
  baseline vazia.
- O shell fica fatiado em módulos menores e o gate impede regressão.
- Testes continuam verdes (shell 352, storage 89, recipes 50, engine 134,
  MCP 12, channels 30).
