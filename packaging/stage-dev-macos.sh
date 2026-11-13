#!/bin/sh
# Local development only: Homebrew helpers and their libraries remain machine dependencies.
set -eu
cd "$(dirname "$0")/.."
[ "$(uname -s)" = Darwin ] || { echo 'Run this on macOS.' >&2; exit 1; }
cargo build --locked
app='dist/uFFmpeg Dev.app'
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/ffmpeg/bin"
cp target/debug/frameforge-desktop "$app/Contents/MacOS/uffmpeg.next"
mv -f "$app/Contents/MacOS/uffmpeg.next" "$app/Contents/MacOS/uffmpeg"
cp packaging/Info.plist "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier dev.uffmpeg.editor.development' "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleName uFFmpeg Dev' "$app/Contents/Info.plist"
cp packaging/icons/uffmpeg.icns "$app/Contents/Resources/uffmpeg.icns"
for tool in ffmpeg ffprobe; do
    helper=$(command -v "$tool") || { echo "Install $tool for local development." >&2; exit 1; }
    ln -sf "$helper" "$app/Contents/Resources/ffmpeg/bin/$tool"
done
# Ad-hoc sign only our app; do not use --deep on linked Homebrew helpers.
codesign --force --sign - "$app"
echo "Local development app: $app (not self-contained or redistributable)"
