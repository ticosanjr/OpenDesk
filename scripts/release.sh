#!/usr/bin/env bash
# Local release: verify, build, collect artifacts, sign and publish.
#
# Usage:
#   scripts/release.sh                  # changed-only verify + fast build
#   scripts/release.sh --full-gate      # full gate (coverage, E2E, goldens) first
#   scripts/release.sh --skip-gate      # trust an external verification
#   scripts/release.sh --skip-build     # reuse the current release binary
#   scripts/release.sh --reuse-build    # reuse binary only if sources are older
#   scripts/release.sh --quality-build  # full LTO, codegen-units=1
#   scripts/release.sh --no-appimage    # skip the AppImage (deb/rpm only)
#   scripts/release.sh --refresh-appimage  # rebuild the AppImage from scratch
#   scripts/release.sh --clean          # prune transient artifacts afterwards
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
QUALITY_BUILD=0
REUSE_BUILD=0
BUNDLE_APPIMAGE=1
REFRESH_APPIMAGE=0
CLEAN_AFTER=0
COMPILE=1

for argument in "$@"; do
  case "$argument" in
    --skip-build) SKIP_BUILD=1 ;;
    --skip-gate) SKIP_GATE=1 ;;
    --full-gate) FULL_GATE=1 ;;
    --quality-build) QUALITY_BUILD=1 ;;
    --reuse-build) REUSE_BUILD=1 ;;
    --no-appimage) BUNDLE_APPIMAGE=0 ;;
    --refresh-appimage) REFRESH_APPIMAGE=1 ;;
    --clean) CLEAN_AFTER=1 ;;
    --help|-h)
      sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown argument: $argument" >&2
      echo "usage: $0 [--skip-build] [--skip-gate] [--full-gate] [--quality-build] [--reuse-build] [--no-appimage] [--refresh-appimage] [--clean]" >&2
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
    echo "==> Running changed-only verification (use --full-gate for coverage, E2E and goldens)"
    "$ROOT/scripts/check-changed.sh"
  fi
else
  echo "==> Release gate skipped (--skip-gate)"
fi

if [[ "$SKIP_BUILD" != "1" && "$REUSE_BUILD" == "1" ]]; then
  BINARY="$ROOT/src-tauri/target/release/${BIN_NAME}"
  newer_source=""
  if [[ -x "$BINARY" ]]; then
    newer_source="$(
      find \
        "$ROOT/src" "$ROOT/public" "$ROOT/starter-packs" \
        "$ROOT/src-tauri/src" "$ROOT/src-tauri/migrations" \
        "$ROOT/src-tauri/capabilities" "$ROOT/src-tauri/icons" \
        "$ROOT/src-tauri/gen" \
        "$ROOT/src-tauri/build.rs" "$ROOT/src-tauri/Cargo.toml" \
        "$ROOT/src-tauri/Cargo.lock" "$ROOT/src-tauri/tauri.conf.json" \
        "$ROOT/src-tauri/update-signing-public-key.txt" \
        "$ROOT/package.json" "$ROOT/package-lock.json" \
        "$ROOT/index.html" "$ROOT/vite.config.ts" \
        "$ROOT/tsconfig.json" "$ROOT/tsconfig.node.json" \
        -type f -newer "$BINARY" -print -quit 2>/dev/null
    )"
  fi
  if [[ -x "$BINARY" && -z "$newer_source" ]]; then
    echo "==> Reusing existing release binary (--reuse-build)"
    COMPILE=0
  elif [[ -n "$newer_source" ]]; then
    echo "    newer than the release binary: ${newer_source#"$ROOT"/}; building anyway"
  else
    echo "    no release binary found; building anyway"
  fi
fi

appimage_arch() {
  case "$(uname -m)" in
    x86_64) printf 'amd64' ;;
    aarch64|arm64) printf 'aarch64' ;;
    armv7l) printf 'armhf' ;;
    *) uname -m ;;
  esac
}

# appimagetool ships inside the linuxdeploy plugin AppImage. Extract it once
# into target/ so later releases only repackage the already-deployed AppDir.
appimage_tool() {
  local tool="$ROOT/src-tauri/target/appimage-tool/squashfs-root/usr/bin/appimagetool"
  if [[ -x "$tool" ]]; then
    printf '%s\n' "$tool"
    return 0
  fi
  local plugin="$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage"
  [[ -x "$plugin" ]] || return 1
  mkdir -p "$ROOT/src-tauri/target/appimage-tool"
  (
    cd "$ROOT/src-tauri/target/appimage-tool" &&
      rm -rf squashfs-root &&
      "$plugin" --appimage-extract >/dev/null 2>&1
  ) || return 1
  [[ -x "$tool" ]] && printf '%s\n' "$tool"
}

# The AppDir holds the system libraries deployed by linuxdeploy. They only need
# to be redeployed when the linked libraries, config or icons change. The
# version is deliberately excluded so a version bump keeps the cache.
appimage_cache_key() {
  local binary="$ROOT/src-tauri/target/release/${BIN_NAME}"
  {
    node -e 'const c=require(process.argv[1]); delete c.version; process.stdout.write(JSON.stringify(c))' \
      "$ROOT/src-tauri/tauri.conf.json" 2>/dev/null | sha256sum | awk '{print $1}'
    if [[ -x "$binary" ]]; then
      ldd "$binary" 2>/dev/null | awk '{print $1}' | sort | sha256sum | awk '{print $1}'
    fi
    find "$ROOT/src-tauri/icons" -type f -print0 2>/dev/null |
      sort -z | xargs -0 sha256sum 2>/dev/null | awk '{print $1}'
    uname -m
  } | sha256sum | awk '{print $1}'
}

# Same marker Tauri's bundler writes for the AppImage target.
patch_appimage_binary() {
  python3 - "$1" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
data = path.read_bytes()
new = b"__TAURI_BUNDLE_TYPE_VAR_APP"
if new in data:
    raise SystemExit(0)
for marker in (b"UNK", b"DEB", b"RPM", b"NSS", b"MSI"):
    old = b"__TAURI_BUNDLE_TYPE_VAR_" + marker
    if data.count(old) == 1:
        path.write_bytes(data.replace(old, new))
        raise SystemExit(0)
raise SystemExit(f"bundle type marker not found in {path}")
PY
}

bundle_appimage() {
  local outdir="$ROOT/src-tauri/target/release/bundle/appimage"
  local appdir="$outdir/OpenDesk.AppDir"
  local keyfile="$outdir/.opendesk-cache-key"
  local key
  key="$(appimage_cache_key)"
  local tool=""

  if [[ "$REFRESH_APPIMAGE" != "1" && -f "$appdir/AppRun" && -f "$keyfile" ]] &&
    [[ "$(cat "$keyfile" 2>/dev/null)" == "$key" ]]; then
    tool="$(appimage_tool || true)"
  fi

  if [[ -n "$tool" ]]; then
    echo "==> Reusing the AppImage AppDir (linuxdeploy skipped)"
    cp -f "$ROOT/src-tauri/target/release/${BIN_NAME}" "$appdir/usr/bin/opendesk"
    patch_appimage_binary "$appdir/usr/bin/opendesk"
    if [[ -x "$ROOT/src-tauri/target/release/opendeskd" ]]; then
      cp -f "$ROOT/src-tauri/target/release/opendeskd" "$appdir/usr/bin/opendeskd"
    fi
    mkdir -p "$outdir"
    ARCH="$(uname -m)" VERSION="$VERSION" \
      "$tool" "$appdir" "$outdir/OpenDesk_${VERSION}_$(appimage_arch).AppImage"
  else
    echo "==> Building the AppImage from scratch (linuxdeploy)"
    npx tauri bundle --bundles appimage
    printf '%s\n' "$key" >"$keyfile"
    tool="$(appimage_tool || true)"
    if [[ -n "$tool" && -x "$ROOT/src-tauri/target/release/opendeskd" ]]; then
      cp -f "$ROOT/src-tauri/target/release/opendeskd" "$appdir/usr/bin/opendeskd"
      ARCH="$(uname -m)" VERSION="$VERSION" \
        "$tool" "$appdir" "$outdir/OpenDesk_${VERSION}_$(appimage_arch).AppImage"
    fi
  fi
}

bundle_linux() {
  echo "==> Bundling deb and rpm"
  npx tauri bundle --bundles deb rpm
  if [[ "$BUNDLE_APPIMAGE" == "1" ]]; then
    bundle_appimage
  else
    echo "    AppImage skipped (--no-appimage)"
  fi
}

if [[ "$SKIP_BUILD" != "1" ]]; then
  if [[ "$COMPILE" == "1" ]]; then
    echo "==> Building OpenDesk v${VERSION} for ${PLATFORM}"
    if [[ "$QUALITY_BUILD" == "1" ]]; then
      echo "    quality profile (full LTO, codegen-units=1)"
      npm run tauri build -- --no-bundle
    else
      echo "    fast profile (thin LTO, codegen-units=16, incremental)"
      CARGO_PROFILE_RELEASE_LTO=thin \
        CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
        CARGO_INCREMENTAL=1 \
        npm run tauri build -- --no-bundle
    fi
  fi

  # The AppImage bundles the daemon too, so build it before packaging.
  if [[ ! -x "$ROOT/src-tauri/target/release/opendeskd" ]]; then
    echo "==> Building opendeskd"
    (
      cd "$ROOT/src-tauri" &&
        CARGO_PROFILE_RELEASE_LTO=thin \
          CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
          CARGO_INCREMENTAL=1 \
          cargo build --release --bin opendeskd
    )
  fi

  case "$PLATFORM" in
    linux-*) bundle_linux ;;
    *) npx tauri bundle ;;
  esac
fi

# Tauri bundles only the desktop binary. Inject the daemon and its systemd user
# unit into the .deb, and build the optional opendeskd RPM when rpmbuild exists.
inject_daemon_into_deb() {
  command -v dpkg-deb >/dev/null 2>&1 || return 0
  local deb daemon unit tmp
  deb="$(find "$ROOT/src-tauri/target/release/bundle/deb" -type f -name "*${VERSION}*.deb" 2>/dev/null | head -n1)"
  daemon="$ROOT/src-tauri/target/release/opendeskd"
  unit="$ROOT/packaging/systemd/opendeskd-user.service"
  [[ -f "$deb" && -x "$daemon" && -f "$unit" ]] || return 0
  tmp="$(mktemp -d)"
  if dpkg-deb -R "$deb" "$tmp" >/dev/null 2>&1; then
    install -Dm755 "$daemon" "$tmp/usr/bin/opendeskd"
    install -Dm644 "$unit" "$tmp/usr/lib/systemd/user/opendeskd.service"
    dpkg-deb --build --root-owner-group "$tmp" "$deb" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp"
}

if [[ "$PLATFORM" == linux-* ]]; then
  inject_daemon_into_deb
  if command -v rpmbuild >/dev/null 2>&1; then
    echo "==> Building opendeskd RPM"
    OPENDESK_VERSION="$VERSION" "$ROOT/scripts/build-daemon-rpm.sh" || \
      echo "release: opendeskd RPM falhou; segue sem ele" >&2
  else
    echo "    opendeskd RPM skipped (rpmbuild ausente)"
  fi
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
    if [[ -x "$ROOT/src-tauri/target/release/opendeskd" ]]; then
      cp -f "$ROOT/src-tauri/target/release/opendeskd" "$DEST/opendeskd-${PLATFORM}"
    fi
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
    if [[ -x "$ROOT/src-tauri/target/release/opendeskd" ]]; then
      cp -f "$ROOT/src-tauri/target/release/opendeskd" "$DEST/opendeskd-${PLATFORM}"
    fi
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
    if [[ -x "$ROOT/src-tauri/target/release/opendeskd.exe" ]]; then
      cp -f "$ROOT/src-tauri/target/release/opendeskd.exe" "$DEST/opendeskd-${PLATFORM}.exe"
    fi
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

if [[ "$CLEAN_AFTER" == "1" ]]; then
  echo "==> Cleaning transient artifacts (build cache preserved)"
  "$ROOT/scripts/clean.sh"
else
  echo "==> Build cache preserved for the next release"
  echo "    use --clean for transient cleanup or scripts/clean.sh --all to drop target/"
fi
