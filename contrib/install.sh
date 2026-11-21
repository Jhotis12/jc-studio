#!/usr/bin/env bash
# Installs JC Studio from an extracted release tarball, or builds it from the repository.
#
# Non-destructive: it does not enable or start the service, and it does not touch the
# database. Run `systemctl --user enable --now jcsd` when you are ready.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BIN_DIR=${JCS_BIN_DIR:-$HOME/.local/bin}
UNIT_DIR=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
DATA_DIR=${XDG_DATA_HOME:-$HOME/.local/share}
APPLICATIONS="$DATA_DIR/applications"
ICONS="$DATA_DIR/icons/hicolor/scalable/apps"

# Tarball layout: binaries and assets next to this script. Repository layout: build first.
if [ -x "$HERE/jcsd" ] && [ -x "$HERE/jc-studio" ]; then
    SOURCE_DIR=$HERE
    ASSET_DIR=$HERE
    echo "installing prebuilt binaries from the tarball"
else
    ROOT=$(cd "$HERE/.." && pwd)
    command -v cargo >/dev/null 2>&1 || export PATH="$HOME/.cargo/bin:$PATH"
    command -v cargo >/dev/null 2>&1 || { echo "install: cargo not found" >&2; exit 1; }
    echo "building release binaries..."
    cargo build --release --manifest-path "$ROOT/Cargo.toml" \
        --bin jcsd --bin jcs --bin jc-studio
    SOURCE_DIR=$ROOT/target/release
    ASSET_DIR=$ROOT/contrib
fi

mkdir -p "$BIN_DIR"
install -m 0755 "$SOURCE_DIR/jcsd" "$BIN_DIR/jcsd"
install -m 0755 "$SOURCE_DIR/jcs" "$BIN_DIR/jcs"
install -m 0755 "$SOURCE_DIR/jc-studio" "$BIN_DIR/jc-studio"

mkdir -p "$UNIT_DIR"
install -m 0644 "$ASSET_DIR/jcsd.service" "$UNIT_DIR/jcsd.service"

# The unit assumes ~/.cargo/bin; rewrite it to the directory actually used.
if [ "$BIN_DIR" != "$HOME/.cargo/bin" ]; then
    sed -i "s|%h/.cargo/bin/jcsd|$BIN_DIR/jcsd|" "$UNIT_DIR/jcsd.service"
fi

# Desktop entry and icon: JC Studio must appear in the application menu, not only work when
# launched from a terminal.
mkdir -p "$APPLICATIONS" "$ICONS"
install -m 0644 "$ASSET_DIR/jc-studio.desktop" "$APPLICATIONS/jc-studio.desktop"
install -m 0644 "$ASSET_DIR/jc-studio.svg" "$ICONS/jc-studio.svg"
sed -i "s|^Exec=.*|Exec=\"$BIN_DIR/jc-studio\"|; s|^TryExec=.*|TryExec=$BIN_DIR/jc-studio|" \
    "$APPLICATIONS/jc-studio.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPLICATIONS" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$DATA_DIR/icons/hicolor" >/dev/null 2>&1 || true
fi

cat <<EOF
installed:
  $BIN_DIR/jcsd
  $BIN_DIR/jcs
  $BIN_DIR/jc-studio
  $UNIT_DIR/jcsd.service
  $APPLICATIONS/jc-studio.desktop
  $ICONS/jc-studio.svg

next:
  systemctl --user daemon-reload
  systemctl --user enable --now jcsd
  jcs doctor

Then open «JC Studio» from the application menu, or run: jc-studio
Make sure $BIN_DIR is on your PATH.
EOF
