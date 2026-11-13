#!/usr/bin/env bash
source "$(dirname "$0")/common.sh"
target=$(native_target)
case "$target" in x86_64-unknown-linux-gnu) arch=amd64 ;; aarch64-unknown-linux-gnu) arch=arm64 ;; *) fail 'Run on a supported Debian/Ubuntu Linux build host.' ;; esac
: "${UFFMPEG_MAINTAINER:?Set a package maintainer name and email}"
[[ "$UFFMPEG_MAINTAINER" != *$'\n'* ]] || fail 'Invalid maintainer'
for tool in cargo dpkg-deb dpkg-shlibdeps tar xz; do need "$tool"; done
vendor="$release_root/packaging/vendor/$target"
"$release_root/packaging/release/audit-helpers.sh" "$vendor" "$target"
cd "$release_root"
mkdir -p work/release dist
stem="uffmpeg_${APP_VERSION}_${arch}"
for suffix in deb source.tar.xz SHA256SUMS; do [[ ! -e "dist/$stem.$suffix" ]] || fail "Output exists: dist/$stem.$suffix"; done
stage=$(mktemp -d "$release_root/work/release/linux.XXXXXX")
"$release_root/packaging/release/collect-records.sh" "$stage/records" "$target"
cargo build --release --locked --target "$target" -p frameforge-desktop
root="$stage/package"
app="$root/usr/lib/uffmpeg"
mkdir -p "$app" "$root/usr/bin" "$root/usr/share/applications" "$root/usr/share/icons/hicolor/256x256/apps" "$root/DEBIAN" "$stage/debian"
cp "target/$target/release/frameforge-desktop" "$app/uffmpeg"
cp -R "$vendor" "$app/ffmpeg"
cp -R "$stage/records/licenses" "$app/licenses"
cp "$stage/records/"{THIRD-PARTY.txt,APP-LICENSE.txt,dependencies.json,SOURCE-LOCATION.txt} "$app/"
ln -s ../lib/uffmpeg/uffmpeg "$root/usr/bin/uffmpeg"
cp packaging/uffmpeg.desktop "$root/usr/share/applications/"
cp packaging/icons/uffmpeg.png "$root/usr/share/icons/hicolor/256x256/apps/"
printf 'Source: uffmpeg\nSection: video\nPriority: optional\nMaintainer: %s\n\nPackage: uffmpeg\nArchitecture: any\nDescription: Native video editor\n' "$UFFMPEG_MAINTAINER" > "$stage/debian/control"
# Resolve linked ABI dependencies from the native package database; fail on unknown libraries.
(
    cd "$stage"
    dpkg-shlibdeps -O "$app/uffmpeg" "$app/ffmpeg/bin/ffmpeg" "$app/ffmpeg/bin/ffprobe"
) > "$stage/shlibs.txt"
linked=$(sed -n 's/^shlibs:Depends=//p' "$stage/shlibs.txt")
[[ -n "$linked" ]] || fail 'Could not derive native package dependencies'
cat > "$root/DEBIAN/control" <<EOT
Package: uffmpeg
Version: $APP_VERSION
Architecture: $arch
Maintainer: $UFFMPEG_MAINTAINER
Section: video
Priority: optional
Depends: $linked, libfontconfig1, libxkbcommon0, libxkbcommon-x11-0, libx11-6, libxcb1, libxcb-shape0, libxcb-xfixes0, libxcb-render0, libxcb-shm0, libxcursor1, libxrandr2, libxi6, libxinerama1, libxrender1, libxfixes3, libwayland-client0, libwayland-cursor0, libwayland-egl1, libegl1, libgl1, fonts-dejavu-core, xdg-utils, xdg-desktop-portal, xdg-desktop-portal-gtk | xdg-desktop-portal-kde
Description: Native Rust and Slint desktop video editor
 Split, merge and extract audio using the included FFmpeg and FFprobe helpers.
EOT
PATH=/usr/bin:/bin "$app/uffmpeg" --diagnostics > "$stage/records/bundled-diagnostics.txt"
dpkg-deb --build --root-owner-group "$root" "$stage/$stem.deb"
tar -cJf "$stage/$stem.source.tar.xz" -C "$stage" records
mv "$stage/$stem.deb" "$stage/$stem.source.tar.xz" "$release_root/dist/"
(
    cd "$release_root/dist"
    for file in "$stem.deb" "$stem.source.tar.xz"; do printf '%s  %s\n' "$(hash_file "$file")" "$file"; done > "$stem.SHA256SUMS"
)
printf 'Created dist/%s.deb. Includes media tools; native OS libraries are declared package dependencies.\n' "$stem"
