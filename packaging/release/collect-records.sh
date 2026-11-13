#!/usr/bin/env bash
source "$(dirname "$0")/common.sh"
[[ $# == 2 ]] || fail 'Usage: collect-records.sh OUTPUT_DIRECTORY TARGET'
output=$1; target=$2
[[ ! -e "$output" ]] || fail 'Use a fresh records directory.'
cd "$release_root"
python3 scripts/collect-upstream-licenses.py "$target" "$output.license-inputs"
export UFFMPEG_LICENSE_INPUTS="$output.license-inputs"
cargo run --release --locked -p frameforge-application --bin release-records -- "$output" "$target"
mkdir -p "$output/source/project" "$output/source/project/.cargo"
# Vendor the exact locked Rust sources for offline rebuilds and license/source review.
cargo vendor --locked --versioned-dirs "$output/source/project/rust-vendor" > "$output/source/project/.cargo/config.toml"
# Cargo emits an absolute directory; make the supplied offline configuration relocatable.
sed 's|directory = ".*"|directory = "rust-vendor"|' "$output/source/project/.cargo/config.toml" > "$output/config.tmp"
mv "$output/config.tmp" "$output/source/project/.cargo/config.toml"
cp Cargo.toml Cargo.lock LICENSE README.md "$output/source/project/"
for directory in apps crates assets docs sprint scripts; do cp -R "$directory" "$output/source/project/"; done
mkdir -p "$output/source/project/packaging"
cp -R packaging/release packaging/icons packaging/Info.plist packaging/uffmpeg.desktop packaging/windows.rc "$output/source/project/packaging/"
cp LICENSE "$output/APP-LICENSE.txt"
rustc -vV > "$output/rust-toolchain.txt"
cargo --version >> "$output/rust-toolchain.txt"
printf 'The companion installer source archive contains records/source/project with exact project and vendored Rust sources. FFmpeg/dav1d source archives are bundled in ffmpeg/source.\n' > "$output/SOURCE-LOCATION.txt"
