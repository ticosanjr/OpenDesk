#!/usr/bin/env bash
# Build the native installers for the current platform without publishing.
# Run it on Linux, macOS or Windows (Git Bash/MSYS). Release publishing stays
# local: use scripts/release.sh on the machine that owns the signing keys.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SKIP_GATE=0
for argument in "$@"; do
  case "$argument" in
    --skip-gate) SKIP_GATE=1 ;;
    *)
      echo "unknown argument: $argument" >&2
      echo "usage: $0 [--skip-gate]" >&2
      exit 2
      ;;
  esac
done

detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux) printf 'linux-%s\n' "$arch" ;;
    Darwin)
      if [[ "$arch" == "arm64" ]]; then
        arch="aarch64"
      fi
      printf 'macos-%s\n' "$arch"
      ;;
    MINGW*|MSYS*|CYGWIN*) printf 'windows-%s\n' "$arch" ;;
    *) printf '%s-%s\n' "$(printf '%s' "$os" | tr '[:upper:]' '[:lower:]')" "$arch" ;;
  esac
}

PLATFORM="${OPENDESK_PLATFORM:-$(detect_platform)}"
VERSION="$(node -p "require('./package.json').version")"
BUNDLE="$ROOT/src-tauri/target/release/bundle"

echo "==> OpenDesk ${VERSION} · ${PLATFORM}"

for tool in node npm cargo; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "build: ${tool} não encontrado no PATH" >&2
    exit 2
  fi
done

case "$PLATFORM" in
  linux-*)
    for tool in pkg-config; do
      command -v "$tool" >/dev/null 2>&1 || {
        echo "build: ${tool} ausente (instale libwebkit2gtk-4.1-dev e libssl-dev)" >&2
        exit 2
      }
    done
    ;;
  macos-*)
    if ! xcode-select -p >/dev/null 2>&1; then
      echo "build: Xcode Command Line Tools ausente (xcode-select --install)" >&2
      exit 2
    fi
    ;;
  windows-*)
    if ! command -v cl.exe >/dev/null 2>&1 && ! command -v link.exe >/dev/null 2>&1; then
      echo "build: MSVC build tools não encontrados; abra o Developer Prompt" >&2
      exit 2
    fi
    ;;
  *)
    echo "build: plataforma ${PLATFORM} não suportada" >&2
    exit 2
    ;;
esac

if [[ "$SKIP_GATE" != "1" ]]; then
  echo "==> Fast gate"
  "$ROOT/scripts/gate-fast.sh"
else
  echo "==> Fast gate skipped (--skip-gate)"
fi

echo "==> tauri build"
npm run tauri build

found=0
case "$PLATFORM" in
  linux-*)
    for directory in deb rpm appimage; do
      [[ -d "$BUNDLE/$directory" ]] && found=1
    done
    ;;
  macos-*)
    [[ -d "$BUNDLE/dmg" || -d "$BUNDLE/macos" ]] && found=1
    ;;
  windows-*)
    [[ -d "$BUNDLE/msi" || -d "$BUNDLE/nsis" ]] && found=1
    ;;
esac

if [[ "$found" != "1" ]]; then
  echo "build: nenhum instalador encontrado em $BUNDLE" >&2
  exit 1
fi

echo "==> Instaladores em $BUNDLE"
find "$BUNDLE" -type f \
  \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \
     -o -name '*.dmg' -o -name '*.msi' -o -name '*.exe' \) \
  -print | sed "s#^$ROOT/##" | sort

echo "==> Build local OK (nada foi publicado)"
echo "    Para publicar com assinatura: scripts/release.sh"
