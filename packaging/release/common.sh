#!/usr/bin/env bash
# Sourced by release scripts; never download prebuilt helpers from an unreviewed source.
set -euo pipefail
release_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
source "$release_root/packaging/release/sources.env"
fail() { printf '%s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null || fail "Install the build tool: $1"; }
hash_file() {
    if command -v sha256sum >/dev/null; then sha256sum "$1" | awk '{print $1}';
    else shasum -a 256 "$1" | awk '{print $1}'; fi
}
native_target() {
    case "$(uname -s):$(uname -m)" in
        Darwin:arm64) printf '%s\n' aarch64-apple-darwin ;;
        Linux:x86_64) printf '%s\n' x86_64-unknown-linux-gnu ;;
        Linux:aarch64) printf '%s\n' aarch64-unknown-linux-gnu ;;
        MINGW*:x86_64|MSYS*:x86_64) printf '%s\n' x86_64-pc-windows-msvc ;;
        *) fail 'Use Apple Silicon, Linux x64/ARM64, or an MSYS2 UCRT64 Windows x64 shell.' ;;
    esac
}
