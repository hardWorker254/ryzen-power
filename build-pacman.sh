#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
version="0.1.0"
pkgname="ryzen-power"
work_dir="$root_dir/dist/pacman-build"
stage_dir="$work_dir/stage/$pkgname-$version"
tarball="$work_dir/$pkgname-$version.tar.gz"

rm -rf "$work_dir"
mkdir -p "$stage_dir"

cp -r "$root_dir/src" "$stage_dir/src"
cp "$root_dir/Cargo.toml" "$stage_dir/Cargo.toml"
cp "$root_dir/Cargo.lock" "$stage_dir/Cargo.lock"
cp "$root_dir/LICENSE" "$stage_dir/LICENSE"
cp "$root_dir/README.md" "$stage_dir/README.md"
cp "$root_dir/config.conf.example" "$stage_dir/config.conf.example"
cp "$root_dir/install.sh" "$stage_dir/install.sh"

tar -C "$work_dir/stage" \
    --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner \
    --use-compress-program="gzip -n" \
    -cf "$tarball" "$pkgname-$version"

checksum="$(sha256sum "$tarball" | cut -d' ' -f1)"

cp "$root_dir/PKGBUILD" "$work_dir/PKGBUILD"
sed -i "s|^sha256sums=.*|sha256sums=('$checksum')|" "$work_dir/PKGBUILD"

cd "$work_dir"
makepkg -f

echo
echo "Built package(s):"
find "$work_dir" -maxdepth 1 -name '*.pkg.tar.zst' -print