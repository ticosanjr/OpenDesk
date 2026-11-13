#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(node -p "require('./package.json').version")"
TAURI_VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
CARGO_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -n 1)"
if [[ "$VERSION" != "$TAURI_VERSION" || "$VERSION" != "$CARGO_VERSION" ]]; then
  echo "release: versão divergente (package=$VERSION tauri=$TAURI_VERSION cargo=$CARGO_VERSION)" >&2
  exit 2
fi
DEST="$ROOT/releases/${VERSION}"
NOTES_FILE="$ROOT/docs/releases/${VERSION}.md"
SKIP_BUILD=0

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
BIN_NAME="opendesk"
case "$PLATFORM" in
  windows-*) BIN_NAME="opendesk.exe" ;;
esac
SKIP_GATE=0
FULL_GATE=0

for argument in "$@"; do
  case "$argument" in
    --skip-build) SKIP_BUILD=1 ;;
    --skip-gate) SKIP_GATE=1 ;;
    --full-gate) FULL_GATE=1 ;;
    *)
      echo "unknown argument: $argument" >&2
      echo "usage: $0 [--skip-build] [--skip-gate] [--full-gate]" >&2
      exit 2
      ;;
  esac
done

if [[ "$SKIP_GATE" != "1" ]]; then
  echo "==> Production dependency audit"
  npm run audit:prod
  if [[ "$FULL_GATE" == "1" ]]; then
    echo "==> Running the full local release gate"
    "$ROOT/scripts/gate.sh"
  else
    echo "==> Running the fast release gate (use --full-gate for coverage and E2E)"
    "$ROOT/scripts/gate-fast.sh"
  fi
else
  echo "==> Release gate skipped (--skip-gate)"
fi

if [[ "$SKIP_BUILD" != "1" ]]; then
  echo "==> Building OpenDesk v${VERSION} for ${PLATFORM}"
  npm run tauri build
fi

echo "==> Collecting v${VERSION} artifacts"
if [[ -f "$DEST/manifest.json" ]]; then
  existing_platform="$(node -e 'console.log(require(process.argv[1]).platform || "")' "$DEST/manifest.json")"
  if [[ -n "$existing_platform" && "$existing_platform" != "$PLATFORM" ]]; then
    echo "release: releases/${VERSION} já foi gerado para ${existing_platform}; use outra versão ou mescle os manifests manualmente" >&2
    exit 2
  fi
fi
rm -rf "$DEST"
mkdir -p "$DEST"

case "$PLATFORM" in
  linux-*)
    for pattern in deb rpm appimage; do
      bundle="$ROOT/src-tauri/target/release/bundle/$pattern"
      [[ -d "$bundle" ]] || continue
      find "$bundle" -type f \
        \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) \
        -name "*${VERSION}*" \
        -exec cp -f {} "$DEST/" \;
    done
    cp -f "$ROOT/src-tauri/target/release/opendesk" "$DEST/opendesk-${PLATFORM}"
    ;;
  macos-*)
    for pattern in dmg macos app; do
      bundle="$ROOT/src-tauri/target/release/bundle/$pattern"
      [[ -d "$bundle" ]] || continue
      find "$bundle" -type f -name "*.dmg" -name "*${VERSION}*" \
        -exec cp -f {} "$DEST/" \;
      find "$bundle" -type f -name "*.app.tar.gz" \
        -exec cp -f {} "$DEST/" \;
    done
    cp -f "$ROOT/src-tauri/target/release/opendesk" "$DEST/opendesk-${PLATFORM}"
    ;;
  windows-*)
    for pattern in msi nsis; do
      bundle="$ROOT/src-tauri/target/release/bundle/$pattern"
      [[ -d "$bundle" ]] || continue
      find "$bundle" -type f \
        \( -name '*.msi' -o -name '*.exe' \) \
        -name "*${VERSION}*" \
        -exec cp -f {} "$DEST/" \;
    done
    cp -f "$ROOT/src-tauri/target/release/opendesk.exe" "$DEST/opendesk-${PLATFORM}.exe"
    ;;
  *)
    echo "release: plataforma ${PLATFORM} sem coleção de artefatos configurada" >&2
    exit 2
    ;;
esac

shopt -s nullglob
case "$PLATFORM" in
  linux-*) installers=("$DEST"/*.deb "$DEST"/*.rpm "$DEST"/*.AppImage) ;;
  macos-*) installers=("$DEST"/*.dmg "$DEST"/*.app.tar.gz) ;;
  windows-*) installers=("$DEST"/*.msi "$DEST"/*setup.exe) ;;
  *) installers=() ;;
esac
shopt -u nullglob
if (( ${#installers[@]} == 0 )); then
  echo "release: nenhum instalador foi gerado em $DEST (rode npm run tauri build antes)" >&2
  exit 1
fi
unset installers

node - "$DEST" "$VERSION" "$NOTES_FILE" "$PLATFORM" <<'NODE'
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const dest = process.argv[2];
const version = process.argv[3];
const notesFile = process.argv[4];
const platform = process.argv[5];
const notes = fs.existsSync(notesFile)
  ? fs.readFileSync(notesFile, "utf8").trim()
  : `OpenDesk v${version}`;
const ignored = new Set(["manifest.json", "SHA256SUMS", "latest.json", "latest.json.sig"]);
const artifacts = fs
  .readdirSync(dest)
  .filter((file) => !ignored.has(file))
  .sort();

function entry(file) {
  const bytes = fs.readFileSync(path.join(dest, file));
  return {
    name: file,
    size: bytes.length,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
  };
}

const manifest = {
  name: "OpenDesk",
  version,
  platform,
  built_at: new Date().toISOString(),
  artifacts,
};
fs.writeFileSync(
  path.join(dest, "manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);

const latest = {
  name: "OpenDesk",
  version,
  tag: `v${version}`,
  repo: "ticosanjr/OpenDesk",
  platform,
  published_at: manifest.built_at,
  notes,
  assets: artifacts.map(entry),
};
fs.writeFileSync(
  path.join(dest, "latest.json"),
  `${JSON.stringify(latest, null, 2)}\n`,
);
NODE

echo "==> Signing update manifest"
SIGNER="$ROOT/src-tauri/target/release/${BIN_NAME}"
if [[ ! -x "$SIGNER" ]]; then
  echo "release: binário $SIGNER ausente; não é possível assinar latest.json" >&2
  exit 1
fi
SIGN_ARGS=("$DEST/latest.json" "$DEST/latest.json.sig")
if [[ -n "${OPENDESK_UPDATE_SIGNING_KEY:-}" ]]; then
  SIGN_ARGS+=(--key "$OPENDESK_UPDATE_SIGNING_KEY")
fi
"$SIGNER" sign-update "${SIGN_ARGS[@]}"
"$SIGNER" verify-update "$DEST/latest.json" "$DEST/latest.json.sig"

node - "$DEST" <<'NODE'
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const dest = process.argv[2];
const files = fs
  .readdirSync(dest)
  .filter((file) => file !== "SHA256SUMS" && fs.statSync(path.join(dest, file)).isFile())
  .sort();
const lines = files.map(
  (file) =>
    `${crypto.createHash("sha256").update(fs.readFileSync(path.join(dest, file))).digest("hex")}  ${file}`,
);
fs.writeFileSync(path.join(dest, "SHA256SUMS"), `${lines.join("\n")}\n`);
NODE

printf '%s\n' "$VERSION" > "$ROOT/releases/LATEST"

echo "==> Keeping at most 3 releases"
node - "$ROOT/releases" <<'NODE'
const fs = require("fs");
const path = require("path");
const root = process.argv[2];
const versions = fs
  .readdirSync(root, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort((left, right) => left.localeCompare(right, undefined, { numeric: true }));
for (const old of versions.slice(0, Math.max(0, versions.length - 3))) {
  console.log(`    prune releases/${old}`);
  fs.rmSync(path.join(root, old), { recursive: true, force: true });
}
NODE

echo "==> Git commit, tag and optional push"
git add -A
git commit -m "release: v${VERSION}" || true
if ! git rev-parse "v${VERSION}" >/dev/null 2>&1; then
  git tag -a "v${VERSION}" -m "OpenDesk v${VERSION}"
fi

remote="$(git remote | head -n 1)"
if [[ -n "$remote" ]]; then
  branch="$(git rev-parse --abbrev-ref HEAD)"
  git push -u "$remote" "$branch"
  git push "$remote" --tags
else
  echo "    no git remote configured; commit and tag kept locally"
fi

echo "==> Publishing GitHub Release v${VERSION}"
if command -v gh >/dev/null 2>&1 && git remote get-url "$remote" 2>/dev/null | grep -q github.com; then
  notes_args=()
  if [[ -f "$NOTES_FILE" ]]; then
    notes_args=(--notes-file "$NOTES_FILE")
  else
    notes_args=(--notes "OpenDesk v${VERSION} (${PLATFORM}). Assets: installers, portable binary, SHA256SUMS and latest.json.")
  fi
  if gh release view "v${VERSION}" >/dev/null 2>&1; then
    echo "    release v${VERSION} already exists; updating notes and assets with --clobber"
    gh release edit "v${VERSION}" --title "OpenDesk v${VERSION}" "${notes_args[@]}"
    gh release upload "v${VERSION}" "$DEST"/* --clobber
  else
    gh release create "v${VERSION}" \
      --title "OpenDesk v${VERSION}" \
      "${notes_args[@]}" \
      "$DEST"/*
  fi
  echo "    https://github.com/$(gh repo view --json nameWithOwner --jq .nameWithOwner)/releases/tag/v${VERSION}"
else
  echo "    gh or a GitHub remote is unavailable; release kept local"
fi

echo "==> Release v${VERSION} ready in releases/${VERSION}"

if [[ "${OPENDESK_SKIP_CLEAN:-0}" != "1" ]]; then
  echo "==> Cleaning build artifacts"
  "$ROOT/scripts/clean.sh"
else
  echo "    cleanup skipped (OPENDESK_SKIP_CLEAN=1)"
fi
