#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(node -p "require('./package.json').version")"
DEST="$ROOT/releases/${VERSION}"
SKIP_BUILD=0
SKIP_GATE=0

for argument in "$@"; do
  case "$argument" in
    --skip-build) SKIP_BUILD=1 ;;
    --skip-gate) SKIP_GATE=1 ;;
    *)
      echo "unknown argument: $argument" >&2
      echo "usage: $0 [--skip-build] [--skip-gate]" >&2
      exit 2
      ;;
  esac
done

if [[ "$SKIP_GATE" != "1" ]]; then
  echo "==> Running the local release gate"
  "$ROOT/scripts/gate.sh"
else
  echo "==> Release gate skipped (--skip-gate)"
fi

if [[ "$SKIP_BUILD" != "1" ]]; then
  echo "==> Building OpenDesk v${VERSION}"
  npm run tauri build
fi

echo "==> Collecting v${VERSION} artifacts"
rm -rf "$DEST"
mkdir -p "$DEST"

for pattern in deb rpm appimage; do
  bundle="$ROOT/src-tauri/target/release/bundle/$pattern"
  [[ -d "$bundle" ]] || continue
  find "$bundle" -maxdepth 1 -type f \
    \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) \
    -name "*${VERSION}*" \
    -exec cp -f {} "$DEST/" \;
done

cp -f "$ROOT/src-tauri/target/release/opendesk" "$DEST/opendesk-linux-x86_64"

node - "$DEST" "$VERSION" <<'NODE'
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const dest = process.argv[2];
const version = process.argv[3];
const ignored = new Set(["manifest.json", "SHA256SUMS", "latest.json"]);
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
  platform: "linux-x86_64",
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
  platform: "linux-x86_64",
  published_at: manifest.built_at,
  notes: `OpenDesk v${version}`,
  assets: artifacts.map(entry),
};
fs.writeFileSync(
  path.join(dest, "latest.json"),
  `${JSON.stringify(latest, null, 2)}\n`,
);
NODE

( cd "$DEST" && sha256sum * > SHA256SUMS )

printf '%s\n' "$VERSION" > "$ROOT/releases/LATEST"

echo "==> Keeping at most 3 releases"
mapfile -t versions < <(
  find "$ROOT/releases" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort -V
)
if (( ${#versions[@]} > 3 )); then
  keep_from=$(( ${#versions[@]} - 3 ))
  for old in "${versions[@]:0:keep_from}"; do
    echo "    prune releases/$old"
    rm -rf "$ROOT/releases/$old"
  done
fi

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
  if gh release view "v${VERSION}" >/dev/null 2>&1; then
    echo "    release v${VERSION} already exists; uploading assets with --clobber"
    gh release upload "v${VERSION}" "$DEST"/* --clobber
  else
    gh release create "v${VERSION}" \
      --title "OpenDesk v${VERSION}" \
      --notes "OpenDesk v${VERSION}. Assets: AppImage, .deb, .rpm, portable binary, SHA256SUMS and latest.json." \
      "$DEST"/*
  fi
  echo "    https://github.com/$(gh repo view --json nameWithOwner --jq .nameWithOwner)/releases/tag/v${VERSION}"
else
  echo "    gh or a GitHub remote is unavailable; release kept local"
fi

echo "==> Release v${VERSION} ready in releases/${VERSION}"
