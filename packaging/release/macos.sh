#!/usr/bin/env bash
source "$(dirname "$0")/common.sh"
mode=${1:-unsigned}
[[ "$mode" == unsigned || "$mode" == signed ]] || fail 'Usage: macos.sh unsigned|signed'
[[ "$(native_target)" == aarch64-apple-darwin ]] || fail 'Run on Apple Silicon macOS.'
for tool in cargo hdiutil codesign ditto xcrun; do need "$tool"; done
if [[ "$mode" == signed ]]; then
    : "${UFFMPEG_SIGN_IDENTITY:?Set a Developer ID Application identity}"
    : "${UFFMPEG_NOTARY_PROFILE:?Set an existing notarytool Keychain profile name}"
fi
target=aarch64-apple-darwin
vendor="$release_root/packaging/vendor/$target"
[[ -d "$vendor" ]] || fail 'Run packaging/release/build-helpers.sh first.'
"$release_root/packaging/release/audit-helpers.sh" "$vendor" "$target"
cd "$release_root"
mkdir -p work/release dist
stem="uFFmpeg-$APP_VERSION-macos-arm64-$mode"
for suffix in dmg source.tar.xz SHA256SUMS; do [[ ! -e "dist/$stem.$suffix" ]] || fail "Output exists: dist/$stem.$suffix"; done
stage=$(mktemp -d "$release_root/work/release/macos.XXXXXX")
"$release_root/packaging/release/collect-records.sh" "$stage/records" "$target"
export MACOSX_DEPLOYMENT_TARGET="$MACOS_MINIMUM"
cargo build --release --locked --target "$target" -p frameforge-desktop
app="$stage/dmg/uFFmpeg.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/licenses"
cp "target/$target/release/frameforge-desktop" "$app/Contents/MacOS/uffmpeg"
cp packaging/Info.plist "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $APP_VERSION" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :LSMinimumSystemVersion $MACOS_MINIMUM" "$app/Contents/Info.plist"
cp packaging/icons/uffmpeg.icns "$app/Contents/Resources/"
cp -R "$vendor" "$app/Contents/Resources/ffmpeg"
# Signing changes executable hashes. Preserve the build-time manifest under an explicit name.
mv "$app/Contents/Resources/ffmpeg/SHA256SUMS" "$app/Contents/Resources/ffmpeg/UNSIGNED-BUILD-SHA256SUMS"
cp -R "$stage/records/licenses/." "$app/Contents/Resources/licenses/"
cp "$stage/records/"{THIRD-PARTY.txt,APP-LICENSE.txt,dependencies.json,SOURCE-LOCATION.txt,rust-toolchain.txt} "$app/Contents/Resources/"
[[ "$(lipo -archs "$app/Contents/MacOS/uffmpeg")" == arm64 ]] || fail 'Wrong app architecture'
dependencies=$(otool -L "$app/Contents/MacOS/uffmpeg")
while IFS= read -r dependency; do
    case "$dependency" in /usr/lib/*|/System/Library/*) ;; *) fail "External app dependency: $dependency" ;; esac
done < <(tail -n +2 <<< "$dependencies" | sed 's/^[[:space:]]*//; s/ (compatibility.*//')
sign_file() {
    if [[ "$mode" == signed ]]; then
        codesign --force --sign "$UFFMPEG_SIGN_IDENTITY" --options runtime --timestamp "$1"
    else
        codesign --force --sign - "$1"
    fi
}
for helper in ffmpeg ffprobe; do sign_file "$app/Contents/Resources/ffmpeg/bin/$helper"; done
sign_file "$app"
codesign --verify --deep --strict "$app"
PATH=/usr/bin:/bin "$app/Contents/MacOS/uffmpeg" --diagnostics > "$stage/records/bundled-diagnostics.txt"
if [[ "$mode" == signed ]]; then
    ditto -c -k --keepParent "$app" "$stage/notarization.zip"
    xcrun notarytool submit "$stage/notarization.zip" --keychain-profile "$UFFMPEG_NOTARY_PROFILE" --wait
    xcrun stapler staple "$app"
    xcrun stapler validate "$app"
fi
ln -s /Applications "$stage/dmg/Applications"
hdiutil create -volname uFFmpeg -srcfolder "$stage/dmg" -format UDZO "$stage/$stem.dmg"
if [[ "$mode" == signed ]]; then
    codesign --sign "$UFFMPEG_SIGN_IDENTITY" --timestamp "$stage/$stem.dmg"
    xcrun notarytool submit "$stage/$stem.dmg" --keychain-profile "$UFFMPEG_NOTARY_PROFILE" --wait
    xcrun stapler staple "$stage/$stem.dmg"
    xcrun stapler validate "$stage/$stem.dmg"
fi
# Keep exact Rust project/dependency sources and build records beside the installer.
tar -cJf "$stage/$stem.source.tar.xz" -C "$stage" records
mv "$stage/$stem.dmg" "$stage/$stem.source.tar.xz" "$release_root/dist/"
(
    cd "$release_root/dist"
    for file in "$stem.dmg" "$stem.source.tar.xz"; do printf '%s  %s\n' "$(hash_file "$file")" "$file"; done > "$stem.SHA256SUMS"
)
printf 'Created dist/%s.dmg and corresponding source/records. Clean-machine installation review is still required.\n' "$stem"
