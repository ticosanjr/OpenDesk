#!/usr/bin/env bash
# Builds the optional opendeskd RPM (daemon + systemd user unit).
#
# The desktop RPM is produced by Tauri. This package is separate so a headless
# machine can install the control plane without the webview/desktop bundle.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${OPENDESK_VERSION:-$(node -p "require('${ROOT}/package.json').version")}"
DAEMON="${1:-$ROOT/src-tauri/target/release/opendeskd}"
UNIT="$ROOT/packaging/systemd/opendeskd-user.service"
SPEC="$ROOT/packaging/rpm/opendeskd.spec"

if ! command -v rpmbuild >/dev/null 2>&1; then
  echo "build-daemon-rpm: rpmbuild não encontrado; instale o pacote rpm" >&2
  exit 1
fi
[[ -x "$DAEMON" ]] || {
  echo "build-daemon-rpm: binário ausente: $DAEMON" >&2
  exit 1
}
[[ -f "$UNIT" ]] || {
  echo "build-daemon-rpm: unit ausente: $UNIT" >&2
  exit 1
}

TOPDIR="$(mktemp -d)"
trap 'rm -rf "$TOPDIR"' EXIT
mkdir -p "$TOPDIR"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
install -Dm755 "$DAEMON" "$TOPDIR/SOURCES/opendeskd"
install -Dm644 "$UNIT" "$TOPDIR/SOURCES/opendeskd.service"

rpmbuild -bb \
  --define "_topdir $TOPDIR" \
  --define "version $VERSION" \
  --define "debug_package %{nil}" \
  "$SPEC"

OUT="$ROOT/src-tauri/target/release/bundle/rpm"
mkdir -p "$OUT"
find "$TOPDIR/RPMS" -type f -name '*.rpm' -exec cp -f {} "$OUT/" \;
echo "build-daemon-rpm: $(find "$OUT" -maxdepth 1 -type f -name 'opendeskd-*.rpm' | tail -n1)"
