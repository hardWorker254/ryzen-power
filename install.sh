#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

cargo build --release

prefix="${PREFIX:-$HOME/.local}"
install_dir="$prefix/bin"
install_path="$install_dir/ryzen-power"

install -Dm755 target/release/ryzen-power "$install_path"

echo "Installed: $install_path"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    echo "Add $install_dir to PATH so the ryzen-power command is available."
    ;;
esac

echo
echo "To build a distribution package instead:"
echo "  ./build-deb.sh      # Debian/Ubuntu: dist/ryzen-power_<version>_amd64.deb"
echo "  ./build-pacman.sh   # Arch/Artix:   dist/pacman-build/ryzen-power-<version>-1-x86_64.pkg.tar.zst"

echo
echo "The first run creates ~/.config/ryzen-power/config.conf:"
echo "  $install_path list"
echo
echo "To apply limits without a password prompt, install the polkit rule once:"
echo "  sudo $install_path install-polkit"
