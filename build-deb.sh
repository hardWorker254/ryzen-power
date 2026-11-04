#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

version="0.1.0-1"
release_binary="target/release/ryzen-power"

if [[ ! -x "$release_binary" ]]; then
    cargo build --release --locked
fi

staging_dir="$(mktemp -d)"
trap 'rm -rf "$staging_dir"' EXIT

mkdir -p \
    "$staging_dir/DEBIAN" \
    "$staging_dir/usr/bin" \
    "$staging_dir/usr/share/doc/ryzen-power"

install -Dm755 "$release_binary" "$staging_dir/usr/bin/ryzen-power"

cat > "$staging_dir/DEBIAN/control" <<EOF
Package: ryzen-power
Version: $version
Section: utils
Priority: optional
Architecture: amd64
Maintainer: ryzen-power maintainers
Installed-Size: 10240
Depends: libgtk-4-1, libgtk4-layer-shell0, polkitd, ryzenadj, libc6, libglib2.0-0
Description: AMD Ryzen power-limit profile switcher with a translucent OSD
 A command line tool that applies AMD Ryzen power limit profiles through ryzenadj
 and displays a translucent OSD while switching profiles.
EOF

cat > "$staging_dir/usr/share/doc/ryzen-power/copyright" <<'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: ryzen-power
Source: local

Files: *
Copyright: 2026 ryzen-power contributors
License: MIT
EOF

mkdir -p dist
output="$(pwd)/dist/ryzen-power_${version}_amd64.deb"

if command -v dpkg-deb >/dev/null 2>&1; then
    dpkg-deb --build "$staging_dir" "$output"
else
    control_dir="$(mktemp -d)"
    trap 'rm -rf "$staging_dir" "$control_dir"' EXIT
    install -Dm644 "$staging_dir/DEBIAN/control" "$control_dir/control"
    tar -C "$control_dir" -czf "$staging_dir/control.tar.gz" ./control

    printf '2.0\n' > "$staging_dir/debian-binary"
    tar -C "$staging_dir" -czf "$staging_dir/data.tar.gz" \
        ./usr

    cd "$staging_dir"
    ar rc "$output" debian-binary control.tar.gz data.tar.gz
    cd /home/prof/proj/ryzen-power
fi

echo "Built: $output"
echo "Members:"
ar t "$output"
