#!/bin/sh
#
# Citadel standalone installer.
#
# Downloads a self-contained bundle from GitHub Releases.
# No Node.js, no build tools, no npm required.
#
#   curl -fsSL https://raw.githubusercontent.com/antonygiomarxdev/citadel/main/install.sh | sh
#
# Upgrade:   re-run the same command.
# Uninstall: curl -fsSL .../install.sh | sh -s -- --uninstall
#
# Environment:
#   CITADEL_VERSION         release tag to install (default: latest)
#   CITADEL_INSTALL_DIR     bundle location   (default: ~/.citadel)
#   CITADEL_BIN_DIR         symlink location  (default: ~/.local/bin)
set -eu

REPO="antonygiomarxdev/citadel"
INSTALL_DIR="${CITADEL_INSTALL_DIR:-$HOME/.citadel}"
BIN_DIR="${CITADEL_BIN_DIR:-$HOME/.local/bin}"

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$BIN_DIR/citadel"
  rm -rf "$INSTALL_DIR"
  echo "Citadel uninstalled (removed $INSTALL_DIR and $BIN_DIR/citadel)."
  exit 0
fi

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin) os="darwin" ;;
  Linux)  os="linux" ;;
  *) echo "citadel: unsupported OS '$os'." >&2; exit 1 ;;
esac
case "$arch" in
  arm64|aarch64) arch="arm64" ;;
  x86_64|amd64)  arch="x64" ;;
  *) echo "citadel: unsupported architecture '$arch'." >&2; exit 1 ;;
esac
target="${os}-${arch}"

version="${CITADEL_VERSION:-}"
if [ -z "$version" ]; then
  version="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest" \
    | sed -n 's#.*/releases/tag/##p')"
fi
if [ -z "$version" ]; then
  version="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n1)"
fi
[ -n "$version" ] || { echo "citadel: could not resolve latest version; set CITADEL_VERSION (e.g. CITADEL_VERSION=v0.8.0)." >&2; exit 1; }
case "$version" in v*) ;; *) version="v$version" ;; esac

url="https://github.com/$REPO/releases/download/$version/citadel-${target}.tar.gz"
echo "Installing Citadel $version ($target)..."
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
curl -fsSL "$url" -o "$tmp/citadel.tar.gz" || { echo "citadel: download failed: $url" >&2; exit 1; }

dest="$INSTALL_DIR/versions/$version"
rm -rf "$dest"
mkdir -p "$dest"
tar -xzf "$tmp/citadel.tar.gz" -C "$dest" --strip-components=1

mkdir -p "$BIN_DIR"
ln -sf "$dest/bin/citadel" "$BIN_DIR/citadel"
ln -sfn "$dest" "$INSTALL_DIR/current"

echo "Installed to $dest"
echo "Linked     $BIN_DIR/citadel"
case ":$PATH:" in
  *":$BIN_DIR:*) ;;
  *)
    echo ""
    echo "$BIN_DIR is not on your PATH. Add it:"
    echo "  export PATH=\"$BIN_DIR:\$PATH\""
    ;;
esac
echo ""
echo "Done. Run: citadel --help"
