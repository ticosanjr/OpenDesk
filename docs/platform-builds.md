# Builds Windows e macOS

OpenDesk publica releases **localmente**, na máquina que tem as chaves de
assinatura. Não há pipeline de release. `npm run build:platform` gera os
instaladores da plataforma atual sem publicar nada; `scripts/release.sh` faz o
manifesto, a assinatura do `latest.json`, a tag e o upload.

## Matriz

A auto-atualização dentro do app é **Linux-only**: o updater substitui o
binário/AppImage em execução. No macOS e Windows a atualização é feita pelo
instalador (`.dmg`/`.msi`/NSIS) baixado da release.

| Plataforma | Requisitos | Instaladores |
|---|---|---|
| Linux | `libwebkit2gtk-4.1-dev`, `libssl-dev`, `pkg-config`, `rpm` (opcional) | `.deb`, `.rpm`, `.AppImage` |
| macOS 12+ | Xcode Command Line Tools | `.dmg`, `.app` |
| Windows 10/11 | MSVC Build Tools, WebView2 Runtime, NSIS (ou WiX para `.msi`) | `.msi`, `.exe` (NSIS) |

O frontend é o mesmo; Rust e Tauri resolvem as diferenças por target. Rode o
build nativo em cada sistema — cross-compile de WebView não é suportado.

## Comandos

```bash
# build + verificação proporcional, sem publicar
npm run build:platform

# build sem repetir a verificação (quando outra máquina já verificou)
npm run build:platform -- --skip-gate

# perfil de build completo (LTO, codegen-units=1) para medir/validar
npm run build:platform -- --quality-build

# release completo na plataforma atual (verify + build + manifesto + tag)
scripts/release.sh

# release pulando o build (artefatos já gerados)
scripts/release.sh --skip-build
```

O release padrão usa o perfil rápido (thin LTO, `codegen-units=16`,
incremental) e preserva `src-tauri/target` para o próximo build. O gate completo
(cobertura, Playwright e goldens) só roda com `scripts/release.sh --full-gate`.
`scripts/clean.sh --all` remove `target` quando o disco precisar.

Os artefatos ficam em `src-tauri/target/release/bundle/` e a coleta do release
usa `releases/<versão>/`. O manifesto registra a plataforma detectada
(`linux-x86_64`, `macos-aarch64`, `windows-x86_64`, …) e `latest.json` usa o
mesmo valor, então um cliente só recebe atualização do seu sistema.

## Assinatura

### macOS

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Sua Empresa (TEAMID)"
export APPLE_ID="conta@empresa.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="TEAMID"
```

Sem essas variáveis o `.dmg` sai sem assinatura/notarização e o Gatekeeper
bloqueia a abertura em outras máquinas. Valide com:

```bash
codesign --verify --deep --strict release/OpenDesk.app
spctl --assess --type execute --verbose release/OpenDesk.app
xcrun notarytool submit OpenDesk.dmg --wait --keychain-profile opendesk
```

### Windows

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "C:\keys\opendesk-windows.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "..."
```

Para Authenticode, informe o certificado no prompt do `tauri build` ou use
`signtool` depois de gerar o `.msi`/`.exe`:

```powershell
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /a OpenDesk_0.51.0_x64-setup.exe
```

### Manifesto de atualização

O updater do OpenDesk tem chave própria, independente do instalador:

```bash
# uma vez por máquina de release; guarde a chave privada fora do repositório
opendesk keygen-update --out "$HOME/.config/opendesk/update-signing.key"

# release.sh assina e verifica latest.json automaticamente
export OPENDESK_UPDATE_SIGNING_KEY="$HOME/.config/opendesk/update-signing.key"
```

`verify-update` roda no fim do release; se falhar, o release não deve ser
publicado.

## Verificação

Antes de publicar em cada sistema:

```bash
npm run check
npm run lint
npm run test:rust
npm run test:ui
npm run build
npm run build:platform
```

O gate completo (`scripts/gate.sh`) e a suíte de features continuam sendo a
referência antes de um release. Playwright e goldens visuais rodam em Linux;
nos outros sistemas valide manualmente as janelas, bandeja, atalhos e o
updater.

## Limites atuais

- Cada execução de `scripts/release.sh` publica o manifesto da plataforma
  atual. Reunir Windows, macOS e Linux na mesma tag exige juntar `manifest.json`
  e `latest.json` manualmente (o updater seleciona o asset pelo nome).
- `scripts/release.sh` recusa plataformas sem coleção de artefatos conhecida.
- A assinatura Apple e o certificado Windows continuam sendo passos locais
  com segredos fora do repositório.
