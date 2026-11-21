#!/usr/bin/env bash
# Installs JC Studio from an extracted release tarball, or builds it from the repository.
#
# Non-destructive: it does not enable or start the service, and it does not touch the
# database. Run `systemctl --user enable --now jcsd` when you are ready.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BIN_DIR=${JCS_BIN_DIR:-$HOME/.local/bin}
UNIT_DIR=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user

# Tarball layout: binaries and unit next to this script. Repository layout: build first.
if [ -x "$HERE/jcsd" ] && [ -x "$HERE/jcs" ]; then
    SOURCE_DIR=$HERE
    echo "installing prebuilt binaries from the tarball"
else
    ROOT=$(cd "$HERE/.." && pwd)
    command -v cargo >/dev/null 2>&1 || export PATH="$HOME/.cargo/bin:$PATH"
    command -v cargo >/dev/null 2>&1 || { echo "install: cargo not found" >&2; exit 1; }
    echo "building release binaries..."
    cargo build --release --manifest-path "$ROOT/Cargo.toml"
    SOURCE_DIR=$ROOT/target/release
    HERE=$ROOT/contrib
fi

mkdir -p "$BIN_DIR"
install -m 0755 "$SOURCE_DIR/jcsd" "$BIN_DIR/jcsd"
install -m 0755 "$SOURCE_DIR/jcs" "$BIN_DIR/jcs"

mkdir -p "$UNIT_DIR"
install -m 0644 "$HERE/jcsd.service" "$UNIT_DIR/jcsd.service"

# The unit assumes ~/.cargo/bin; rewrite it to the directory actually used.
if [ "$BIN_DIR" != "$HOME/.cargo/bin" ]; then
    sed -i "s|%h/.cargo/bin/jcsd|$BIN_DIR/jcsd|" "$UNIT_DIR/jcsd.service"
fi

cat <<EOF
installed:
  $BIN_DIR/jcsd
  $BIN_DIR/jcs
  $UNIT_DIR/jcsd.service

next:
  systemctl --user daemon-reload
  systemctl --user enable --now jcsd
  jcs doctor

Make sure $BIN_DIR is on your PATH.
EOF
