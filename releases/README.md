# Releases

Artefatos de release do OpenDesk são gerados **localmente** por
`scripts/release.sh`. Não há GitHub Actions nem pipeline de CI.

## Política

- Cada versão fica em `releases/<versão>/`.
- O script mantém no máximo **as 3 versões mais recentes** (ordenação semver).
- Binários e instaladores são ignorados pelo git; `manifest.json` e
  `SHA256SUMS` são versionados.
- `releases/LATEST` aponta para a versão mais recente.
- O script cria commit e tag `v<versão>` e faz `git push` se houver remoto
  configurado. Sem remoto, o commit/tag ficam locais.

## Conteúdo esperado de uma release Linux

```text
releases/0.2.0/
├── OpenDesk_0.2.0_amd64.deb
├── OpenDesk-0.2.0-1.x86_64.rpm
├── OpenDesk_0.2.0_amd64.AppImage
├── opendesk-linux-x86_64
├── manifest.json
└── SHA256SUMS
```

## Comandos

```bash
scripts/release.sh              # build + empacotamento + retenção + git
scripts/release.sh --skip-build # reempacota artefatos já compilados
```

Pré-requisitos: Rust, Node, dependências do Tauri no Linux e `jq`/`sha256sum`
para o manifesto/checksums.
