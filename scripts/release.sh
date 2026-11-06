#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(node -p "require('./package.json').version")"
DEST="$ROOT/releases/${VERSION}"
SKIP_BUILD="${1:-}"

if [[ "$SKIP_BUILD" != "--skip-build" ]]; then
  echo "==> Building OpenDesk v${VERSION}"
  npm run tauri build
fi

echo "==> Collecting v${VERSION} artifacts"
mkdir -p "$DEST"

for pattern in deb rpm appimage; do
  bundle="$ROOT/src-tauri/target/release/bundle/$pattern"
  [[ -d "$bundle" ]] || continue
  find "$bundle" -maxdepth 1 -type f \
    \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) \
    -exec cp -f {} "$DEST/" \;
done

cp -f "$ROOT/src-tauri/target/release/opendesk" "$DEST/opendesk-linux-x86_64"

node - "$DEST" "$VERSION" <<'NODE'
const fs = require("fs");
const path = require("path");
const dest = process.argv[2];
const version = process.argv[3];
const ignored = new Set(["manifest.json", "SHA256SUMS"]);
const artifacts = fs
  .readdirSync(dest)
  .filter((file) => !ignored.has(file))
  .sort();
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

if git remote | grep -q .; then
  git push
  git push --tags
else
  echo "    no git remote configured; commit and tag kept locally"
fi

echo "==> Release v${VERSION} ready in releases/${VERSION}"
