pkgbase=ryzen-power
pkgname=ryzen-power
pkgver=0.1.0
pkgrel=1
pkgdesc="AMD Ryzen power-limit profile switcher with a translucent OSD"
arch=('x86_64')
url="https://github.com/hardWorker254/ryzen-power"
license=('MIT')
depends=('gtk4' 'gtk4-layer-shell' 'polkit' 'ryzenadj')
makedepends=('rust' 'gtk4-layer-shell')
source=("ryzen-power-0.1.0.tar.gz")
sha256sums=('SKIP')

build() {
    cd "${srcdir}/ryzen-power-0.1.0"
    cargo build --release --locked
}

package() {
    install -Dm755 "${srcdir}/ryzen-power-0.1.0/target/release/ryzen-power" "${pkgdir}/usr/bin/ryzen-power"
}