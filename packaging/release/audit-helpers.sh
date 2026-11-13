#!/usr/bin/env bash
source "$(dirname "$0")/common.sh"
[[ $# == 2 ]] || fail 'Usage: audit-helpers.sh VENDOR_DIRECTORY TARGET'
vendor=$(cd "$1" && pwd)
target=$2
ext=''; [[ "$target" != *windows* ]] || ext=.exe
for record in build-info.txt corresponding-source.txt SHA256SUMS; do [[ -s "$vendor/$record" ]] || fail "Missing $record"; done
for file in "$vendor/licenses/FFmpeg-LGPL-2.1.txt" "$vendor/licenses/dav1d-COPYING" "$vendor/source/ffmpeg-$FFMPEG_VERSION.tar.xz" "$vendor/source/dav1d-$DAV1D_VERSION.tar.xz" "$vendor/source/zlib-$ZLIB_VERSION.tar.xz" "$vendor/licenses/zlib-LICENSE.h"; do
    [[ -s "$file" && ! -L "$file" ]] || fail "Missing regular release input: $file"
done
while read -r checksum file; do
    [[ "$file" == bin/* || "$file" == source/* ]] || fail 'Unexpected checksum path'
    [[ "$file" != *..* && -f "$vendor/$file" && ! -L "$vendor/$file" ]] || fail 'Invalid checksum file'
    [[ "$(hash_file "$vendor/$file")" == "$checksum" ]] || fail "Checksum mismatch: $file"
done < "$vendor/SHA256SUMS"
for name in ffmpeg ffprobe; do
    grep -Fq "  bin/$name$ext" "$vendor/SHA256SUMS" || fail "Missing checksum for $name"
    binary="$vendor/bin/$name$ext"
    [[ -f "$binary" && ! -L "$binary" && -x "$binary" ]] || fail "Missing regular executable: $binary"
    version=$("$binary" -version)
    read -r reported_name version_label reported_version remainder <<< "$version"
    [[ "$reported_name" == "$name" && "$version_label" == version && "${reported_version#n}" == "$FFMPEG_VERSION" ]] || fail "Wrong $name version: $reported_version (expected $FFMPEG_VERSION or n$FFMPEG_VERSION)"
    [[ "$version" != *--enable-gpl* && "$version" != *--enable-nonfree* && "$version" != *--enable-version3* ]] || fail 'Unapproved FFmpeg license configuration'
    [[ "$version" == *--disable-autodetect* && "$version" == *--enable-libdav1d* ]] || fail 'Unexpected helper configuration'
    case "$target" in
        aarch64-apple-darwin)
            [[ "$(lipo -archs "$binary")" == arm64 ]] || fail 'Wrong helper architecture'
            dependencies=$(otool -L "$binary")
            while IFS= read -r dependency; do
                case "$dependency" in /usr/lib/*|/System/Library/*) ;; *) fail "External helper dependency: $dependency" ;; esac
            done < <(tail -n +2 <<< "$dependencies" | sed 's/^[[:space:]]*//; s/ (compatibility.*//') ;;
        *linux*)
            need readelf
            case "$target" in
                x86_64*) readelf -h "$binary" | grep -q 'Advanced Micro Devices X86-64' || fail 'Wrong x64 helper' ;;
                aarch64*) readelf -h "$binary" | grep -q AArch64 || fail 'Wrong ARM64 helper' ;;
                *) fail 'Unsupported Linux target' ;;
            esac
            dynamic=$(readelf -d "$binary")
            while read -r dependency; do
                case "$dependency" in libc.so.*|libm.so.*|libpthread.so.*|libdl.so.*|librt.so.*|libgcc_s.so.*|ld-linux*) ;; *) fail "Non-baseline helper library: $dependency" ;; esac
            done < <(sed -n 's/.*Shared library: \[\(.*\)\]/\1/p' <<< "$dynamic") ;;
        *windows*)
            need objdump
            objdump -f "$binary" | grep -q 'pei-x86-64' || fail 'Wrong Windows helper architecture'
            imports=$(objdump -p "$binary")
            while read -r dependency; do
                case "$dependency" in kernel32.dll|user32.dll|advapi32.dll|shell32.dll|ole32.dll|ws2_32.dll|bcrypt.dll|secur32.dll|ntdll.dll|ucrtbase.dll|msvcrt.dll|api-ms-win-*.dll) ;; *) fail "Unbundled Windows helper library: $dependency" ;; esac
            done < <(awk '/DLL Name:/{print tolower($3)}' <<< "$imports") ;;
        *) fail 'Unsupported target' ;;
    esac
done
ffmpeg="$vendor/bin/ffmpeg$ext"
encoders=$("$ffmpeg" -hide_banner -encoders 2>&1)
for codec in mpeg4 aac pcm_s16le ffv1 png; do grep -Eq "[[:space:]]$codec[[:space:]]" <<< "$encoders" || fail "Missing encoder: $codec"; done
decoders=$("$ffmpeg" -hide_banner -decoders 2>&1)
for codec in h264 hevc libdav1d aac opus; do grep -Eq "[[:space:]]$codec[[:space:]]" <<< "$decoders" || fail "Missing decoder: $codec"; done
muxers=$("$ffmpeg" -hide_banner -muxers 2>&1)
for muxer in mp4 matroska ipod wav; do grep -Eq "[[:space:]]$muxer[[:space:]]" <<< "$muxers" || fail "Missing muxer: $muxer"; done
filters=$("$ffmpeg" -hide_banner -filters 2>&1)
for filter in scale pad setsar fps tpad trim setpts aresample aformat apad atrim asetpts anullsrc; do grep -Eq "[[:space:]]$filter[[:space:]]" <<< "$filters" || fail "Missing filter: $filter"; done
scale_help=$("$ffmpeg" -hide_banner -h filter=scale 2>&1)
grep -q reset_sar <<< "$scale_help" || fail 'Missing scale reset_sar support'
printf 'Helper dependency and capability audit passed for %s\n' "$target"
