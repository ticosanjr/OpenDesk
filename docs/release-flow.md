# Fluxo rápido de verificação e release

O gate completo (`scripts/gate.sh`) existe para fechamentos de release e para
quando o usuário pedir uma verificação profunda. O dia a dia usa verificação
proporcional.

## Verificação proporcional

```bash
npm run verify            # alias de npm run check:changed
scripts/gate-fast.sh --changed
```

`scripts/check-changed.sh` olha apenas o que está diferente em relação a `HEAD`
(staged, unstaged e arquivos novos):

| Mudança | O que roda |
|---|---|
| só docs/config | `git diff --check` |
| `.rs`, `Cargo.toml`, `Cargo.lock` | `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test --lib` |
| `src/`, `tests/`, configs de frontend | ESLint, `tsc --noEmit`, Vitest |
| ambos | as duas colunas |

Nunca roda cobertura, Playwright ou goldens. O tempo total aparece no fim
(`Changed-only verification passed in Ns`).

`npm run gate:fast` continua disponível para uma checagem ampla sem cobertura e
sem E2E; `scripts/gate-fast.sh --rust` inclui clippy e testes Rust.

## Release padrão

```bash
scripts/release.sh
```

1. Confere que `package.json`, `tauri.conf.json` e `Cargo.toml` têm a mesma
   versão.
2. Roda `npm run audit:prod` e `scripts/check-changed.sh`.
3. Compila com o perfil rápido: thin LTO, `codegen-units=16`, incremental.
4. Empacota `.deb` e `.rpm` (segundos). O AppImage reutiliza o AppDir já
   montado e roda só o empacotador; `linuxdeploy` só roda quando dependências,
   configuração ou ícones mudam.
5. Coleta os artefatos, assina `latest.json`, poda releases antigas e faz
   commit, tag e push.
6. **Não apaga `src-tauri/target`**: o próximo release recompila só o que mudou.

Flags:

| Flag | Efeito |
|---|---|
| `--full-gate` | roda `scripts/gate.sh` (cobertura, E2E e goldens) antes do build |
| `--skip-gate` | confia em verificação externa |
| `--skip-build` | usa os artefatos já presentes em `target/release` |
| `--reuse-build` | pula a compilação se nenhum fonte/config for mais novo que o binário (ainda empacota) |
| `--quality-build` | LTO completo e `codegen-units=1` |
| `--no-appimage` | gera só `.deb`/`.rpm` |
| `--refresh-appimage` | refaz o AppImage do zero com `linuxdeploy` |
| `--clean` | limpeza transitória no fim (preserva o cache) |

## AppImage

O `linuxdeploy` reaplica rpath/patchelf em cerca de 500 bibliotecas do sistema a
cada execução (~5 min), mesmo quando elas não mudaram. O release guarda o AppDir
em `src-tauri/target/release/bundle/appimage/` e uma chave calculada com a lista
de bibliotecas do binário (`ldd`), a configuração sem a versão e os ícones. Se a
chave confere, ele apenas:

1. copia o novo binário para `AppDir/usr/bin/opendesk`;
2. grava o marcador de bundle `APP` que o Tauri usaria;
3. roda `appimagetool` (~9 s).

Quando a chave muda (nova dependência nativa, config ou ícone), o AppImage é
refeito pelo `linuxdeploy` e a chave é atualizada. `--refresh-appimage` força
isso manualmente.

## Cache e limpeza

`scripts/clean.sh` remove `dist`, `coverage`, relatórios e arquivos temporários,
mas preserva `src-tauri/target`. Para abrir espaço:

```bash
npm run clean        # transitório, preserva o cache
npm run clean:all    # remove src-tauri/target (próximo build é frio)
npm run clean:deep   # remove target e node_modules
```

O primeiro release depois de um `clean:all`/`clean:deep` paga o build frio; os
seguintes aproveitam o cache.

## Tempos medidos

Medições locais (12 núcleos, Linux, cache em `src-tauri/target`):

| Cenário | Tempo |
|---|---|
| `npm run verify` com Rust + frontend alterados | ~62 s |
| Build release do zero (dependências + app, perfil rápido) | 6 min 34 s |
| Build release sem mudanças (cache quente) | ~1 s |
| Build após mudar um arquivo Rust | 1 min 28 s–1 min 35 s |
| Empacotar `.deb` + `.rpm` | 4 s |
| AppImage reutilizando o AppDir | 9 s |
| AppImage do zero (`linuxdeploy`) | 5 min 37 s |
| Release típico com uma mudança Rust | ~3 min |
| Release só de docs com `--reuse-build` | ~30 s |

O build de release frio acontece uma vez após `clean:all`/`clean:deep`; depois
disso cada release recompila apenas o app, empacota em segundos e preserva o
AppDir. O `linuxdeploy` completo só volta a rodar quando a chave do AppImage
muda.

