#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(node -p "require('./package.json').version")"
DEST="$ROOT/releases/${VERSION}"
NOTES_FILE="$ROOT/docs/releases/${VERSION}.md"
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
  echo "==> Production dependency audit"
  npm run audit:prod
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

node - "$DEST" "$VERSION" "$NOTES_FILE" <<'NODE'
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const dest = process.argv[2];
const version = process.argv[3];
const notesFile = process.argv[4];
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
  notes,
  assets: artifacts.map(entry),
};
fs.writeFileSync(
  path.join(dest, "latest.json"),
  `${JSON.stringify(latest, null, 2)}\n`,
);
NODE

echo "==> Signing update manifest"
SIGNER="$ROOT/src-tauri/target/release/opendesk"
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

(
  cd "$DEST"
  find . -maxdepth 1 -type f ! -name 'SHA256SUMS' -printf '%f\n' \
    | sort \
    | xargs sha256sum > SHA256SUMS
)

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
  notes_args=()
  if [[ -f "$NOTES_FILE" ]]; then
    notes_args=(--notes-file "$NOTES_FILE")
  else
    notes_args=(--notes "OpenDesk v${VERSION}. Assets: AppImage, .deb, .rpm, portable binary, SHA256SUMS and latest.json.")
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
