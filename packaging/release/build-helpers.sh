#!/usr/bin/env bash
# Builds only when explicitly invoked by the developer. No tests or system-wide installation.
source "$(dirname "$0")/common.sh"
for tool in git make meson ninja pkg-config tar xz rustc; do need "$tool"; done
target=$(native_target)
[[ "$release_root" != *[[:space:]]* ]] || fail 'Build native helpers from a project path without spaces (for example C:/src/uFFmpeg).'
[[ "$target" != x86_64* ]] || need nasm
jobs=${BUILD_JOBS:-2}
[[ "$jobs" =~ ^[1-9][0-9]*$ ]] || fail 'BUILD_JOBS must be positive.'
work="$release_root/work/release/$target"
vendor="$release_root/packaging/vendor/$target"
[[ ! -e "$vendor" ]] || fail "Vendor output already exists: $vendor. Move it aside before rebuilding."
mkdir -p "$work"
# Recover a completed compilation rejected by the final audit without rebuilding it.
if [[ $# != 0 ]]; then
    [[ $# == 2 && "$1" == --resume ]] || fail 'Usage: build-helpers.sh [--resume BUILD_DIRECTORY]'
    resume=$(cd "$2" && pwd)
    [[ "$(dirname "$resume")" == "$work" && "$(basename "$resume")" == helpers.* ]] || fail 'Resume requires a helper build directory for this native target.'
    [[ "$(git -C "$resume/ffmpeg" rev-parse HEAD)" == "$FFMPEG_COMMIT" ]] || fail 'Wrong resumed FFmpeg revision'
    [[ "$(git -C "$resume/dav1d" rev-parse HEAD)" == "$DAV1D_COMMIT" ]] || fail 'Wrong resumed dav1d revision'
    [[ "$(git -C "$resume/zlib" rev-parse HEAD)" == "$ZLIB_COMMIT" ]] || fail 'Wrong resumed zlib revision'
    cp "$release_root/packaging/release/"{build-helpers.sh,audit-helpers.sh,common.sh,sources.env} "$resume/vendor/source/"
    "$release_root/packaging/release/audit-helpers.sh" "$resume/vendor" "$target"
    mv "$resume/vendor" "$vendor"
    printf 'Recovered completed helpers: %s\n' "$vendor"
    exit 0
fi
# Fresh private build directory prevents stale configuration and source modifications leaking in.
build=$(mktemp -d "$work/helpers.XXXXXX")
prefix="$build/prefix"
fetch_source() {
    git init -q "$build/$1"
    git -C "$build/$1" remote add origin "$2"
    git -C "$build/$1" fetch --depth 1 origin "refs/tags/$4:refs/tags/$4"
    git -C "$build/$1" checkout --detach "$4"
    [[ "$(git -C "$build/$1" rev-parse HEAD)" == "$3" ]] || fail "Wrong $1 source revision"
}
fetch_source ffmpeg "$FFMPEG_REPOSITORY" "$FFMPEG_COMMIT" "n$FFMPEG_VERSION"
fetch_source dav1d "$DAV1D_REPOSITORY" "$DAV1D_COMMIT" "$DAV1D_VERSION"
fetch_source zlib "$ZLIB_REPOSITORY" "$ZLIB_COMMIT" "v$ZLIB_VERSION"
SOURCE_DATE_EPOCH=$(git -C "$build/ffmpeg" show -s --format=%ct HEAD)
export SOURCE_DATE_EPOCH
flags=(--disable-autodetect --disable-gpl --disable-nonfree --disable-version3
       --disable-shared --enable-static --disable-doc --disable-debug --disable-ffplay
       --disable-network --enable-zlib --enable-libdav1d --pkg-config-flags=--static)
ext=''
case "$target" in
    aarch64-apple-darwin)
        export MACOSX_DEPLOYMENT_TARGET="$MACOS_MINIMUM"
        export CC="${CC:-clang}"
        export CFLAGS="-mmacosx-version-min=$MACOS_MINIMUM"
        export LDFLAGS="$CFLAGS"
        flags+=(--arch=aarch64 --target-os=darwin) ;;
    *windows*)
        [[ "${MSYSTEM:-}" == UCRT64 ]] || fail 'Build helpers in MSYS2 UCRT64.'
        export CC=gcc
        export LDFLAGS='-static -static-libgcc'
        flags+=(--arch=x86_64 --target-os=mingw32)
        ext=.exe ;;
    *) export CC="${CC:-cc}" ;;
esac
need "$CC"
# Keep compression/PNG support local to the helper build as well.
mkdir -p "$prefix/lib/pkgconfig" "$prefix/include"
(
    cd "$build/zlib"
    if [[ "$target" == *windows* ]]; then
        make -f win32/Makefile.gcc -j "$jobs" libz.a
        cp libz.a "$prefix/lib/"
        cp zlib.h zconf.h "$prefix/include/"
    else
        ./configure --static --prefix="$prefix"
        make -j "$jobs" libz.a
        cp libz.a "$prefix/lib/"
        cp zlib.h zconf.h "$prefix/include/"
    fi
)
cat > "$prefix/lib/pkgconfig/zlib.pc" <<EOT
prefix=$prefix
libdir=\${prefix}/lib
includedir=\${prefix}/include
Name: zlib
Description: Pinned static zlib
Version: $ZLIB_VERSION
Libs: -L\${libdir} -lz
Cflags: -I\${includedir}
EOT
# dav1d supplies CPU AV1 decoding without adding a GPL encoder.
meson setup "$build/dav1d-build" "$build/dav1d" --prefix "$prefix" --libdir lib \
    --buildtype release --default-library static -Denable_tools=false -Denable_tests=false \
    -Denable_examples=false -Denable_docs=false
meson compile -C "$build/dav1d-build" -j "$jobs"
meson install -C "$build/dav1d-build"
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"
export PKG_CONFIG_PATH=''
mkdir -p "$build/ffmpeg-build" "$build/vendor/bin" "$build/vendor/licenses" "$build/vendor/source"
(
    cd "$build/ffmpeg-build"
    "$build/ffmpeg/configure" "${flags[@]}" --cc="$CC" \
        --extra-cflags="${CFLAGS:-} -I$prefix/include" \
        --extra-ldflags="${LDFLAGS:-} -L$prefix/lib" --prefix="$prefix"
    make -j "$jobs" ffmpeg"$ext" ffprobe"$ext"
    cp ffmpeg"$ext" ffprobe"$ext" "$build/vendor/bin/"
    cp config.h config_components.h ffbuild/config.mak "$build/vendor/source/"
)
cp "$build/ffmpeg/COPYING.LGPLv2.1" "$build/vendor/licenses/FFmpeg-LGPL-2.1.txt"
cp "$build/ffmpeg/LICENSE.md" "$build/vendor/licenses/FFmpeg-LICENSE.md"
cp "$build/dav1d/COPYING" "$build/vendor/licenses/dav1d-COPYING"
cp "$build/zlib/zlib.h" "$build/vendor/licenses/zlib-LICENSE.h"
git -C "$build/ffmpeg" archive --format=tar --prefix=ffmpeg/ HEAD | xz -T "$jobs" > "$build/vendor/source/ffmpeg-$FFMPEG_VERSION.tar.xz"
git -C "$build/dav1d" archive --format=tar --prefix=dav1d/ HEAD | xz -T "$jobs" > "$build/vendor/source/dav1d-$DAV1D_VERSION.tar.xz"
git -C "$build/zlib" archive --format=tar --prefix=zlib/ HEAD | xz -T "$jobs" > "$build/vendor/source/zlib-$ZLIB_VERSION.tar.xz"
cp "$release_root/packaging/release/"{build-helpers.sh,audit-helpers.sh,common.sh,sources.env} "$build/vendor/source/"
{
    printf 'Target: %s\nFFmpeg: %s %s\ndav1d: %s %s\n' "$target" "$FFMPEG_VERSION" "$FFMPEG_COMMIT" "$DAV1D_VERSION" "$DAV1D_COMMIT"
    printf 'zlib: %s %s\n' "$ZLIB_VERSION" "$ZLIB_COMMIT"
    printf 'Configure:'; printf ' %q' "${flags[@]}"; printf '\n'
    "$CC" --version
    meson --version
    rustc --version
    uname -a
    [[ "$target" != aarch64-apple-darwin ]] || xcrun --show-sdk-version
    "$build/vendor/bin/ffmpeg$ext" -buildconf
} > "$build/vendor/build-info.txt" 2>&1
cat > "$build/vendor/corresponding-source.txt" <<EOT
Exact unmodified source archives, configuration and helper build scripts are in source/ beside this file.
FFmpeg: $FFMPEG_REPOSITORY at $FFMPEG_COMMIT
Dav1d: $DAV1D_REPOSITORY at $DAV1D_COMMIT
Zlib: $ZLIB_REPOSITORY at $ZLIB_COMMIT
Rebuild from the uFFmpeg source project using packaging/release/build-helpers.sh.
FFmpeg uses LGPL-2.1-or-later terms for this configuration. Dav1d license text is included.
Review all release obligations before distribution; this record is not a legal certification.
EOT
(
    cd "$build/vendor"
    for file in bin/* source/*.tar.xz; do printf '%s  %s\n' "$(hash_file "$file")" "$file"; done > SHA256SUMS
)
# Promotion occurs only after all native audit/capability checks pass.
"$release_root/packaging/release/audit-helpers.sh" "$build/vendor" "$target"
mv "$build/vendor" "$vendor"
printf 'Helpers ready: %s\n' "$vendor"
