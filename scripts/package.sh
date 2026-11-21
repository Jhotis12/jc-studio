#!/usr/bin/env bash
# Builds a Linux release tarball: daemon, CLI, desktop application, installer and docs.
#
# Deliberately a script, not a distribution framework: the artifact is three binaries plus
# desktop metadata, and a script a reviewer can read beats a generator nobody inspects.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
command -v cargo >/dev/null 2>&1 || export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "package: cargo not found" >&2; exit 1; }

VERSION=$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT/Cargo.toml" \
    | python3 -c "import json,sys; print(json.load(sys.stdin)['packages'][0]['version'])")
TARGET=$(rustc -vV | awk '/^host:/{print $2}')
NAME="jc-studio-${VERSION}-${TARGET}"
OUT="$ROOT/dist"
STAGE="$OUT/$NAME"

echo "packaging $NAME"

rm -rf "$STAGE"
mkdir -p "$STAGE"

cargo build --release --locked --manifest-path "$ROOT/Cargo.toml" \
    --bin jcsd --bin jcs --bin jc-studio

install -m 0755 "$ROOT/target/release/jcsd" "$STAGE/jcsd"
install -m 0755 "$ROOT/target/release/jcs" "$STAGE/jcs"
install -m 0755 "$ROOT/target/release/jc-studio" "$STAGE/jc-studio"

install -m 0644 "$ROOT/README.md" "$STAGE/README.md"
install -m 0644 "$ROOT/docs/RUNBOOK.md" "$STAGE/RUNBOOK.md"
install -m 0755 "$ROOT/contrib/install.sh" "$STAGE/install.sh"
install -m 0644 "$ROOT/contrib/jcsd.service" "$STAGE/jcsd.service"
install -m 0644 "$ROOT/contrib/jc-studio.desktop" "$STAGE/jc-studio.desktop"
install -m 0644 "$ROOT/contrib/jc-studio.svg" "$STAGE/jc-studio.svg"
cp -r "$ROOT/contrib/licenses" "$STAGE/licenses"

cat > "$STAGE/INSTALL.txt" <<'EOF'
JC Studio — instalación desde este tarball

  ./install.sh              # binarios, unidad systemd y entrada de menú
  systemctl --user daemon-reload
  systemctl --user enable --now jcsd
  jcs doctor

Después abre «JC Studio» desde el menú de aplicaciones. El instalador usa los binarios que
están junto a este archivo; no compila nada.
EOF

tar -czf "$OUT/$NAME.tar.gz" -C "$OUT" "$NAME"
sha256sum "$OUT/$NAME.tar.gz" > "$OUT/$NAME.tar.gz.sha256"

rm -rf "$STAGE"

echo
echo "artifact: $OUT/$NAME.tar.gz"
echo "checksum: $(cut -d' ' -f1 "$OUT/$NAME.tar.gz.sha256")"
echo "size:     $(du -h "$OUT/$NAME.tar.gz" | cut -f1)"
