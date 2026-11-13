# Self-contained distribution

Release is not yet verified. Phase 8 command implementations are in [packaging/release](../packaging/release/README.md); they have not been executed end to end. Never copy Homebrew executables blindly: they may reference external dylibs and enable GPL or nonredistributable components. Development PATH tools are for local inspection only.

## FFmpeg build policy

Pin an upstream release or commit and source hash per platform; record exact configure flags, compiler/SDK versions, patches and dependency hashes. Initial candidate: --disable-gpl --disable-nonfree --disable-doc --disable-debug, retaining ffmpeg and ffprobe. Audit the resulting build, not only the intended flags. Do not enable libx264/libx265 without a deliberate GPL distribution decision. Native AAC and available platform encoders can inform later presets; enumerate actual encoders before choosing defaults. Codec patent and distribution obligations depend on build and jurisdiction.

Ship the exact applicable license texts, third-party notices, corresponding FFmpeg source and patches/build scripts or a compliant source delivery mechanism. Preserve copyrights and attribution in About and release download pages. Separate processes do not remove obligations for redistributed helpers. App code is MIT; Slint's runtime has separate licensing. Use its royalty-free desktop terms with accessible AboutSlint attribution (present in the shell), or deliberately select another applicable Slint license before shipping. Review exact pinned dependency terms.

## Platform release gates

- macOS: build helpers for arm64 with an explicit minimum deployment target. Audit file/lipo and otool -L recursively; bundle non-system dylibs and fix install names or use vetted self-contained helper builds. Copy resources, sign nested libraries/helpers then the app with Developer ID and hardened runtime, notarize, staple, and create/sign DMG. Do not use ad-hoc signing for distribution.
- Windows: matching x64/ARM64 helpers and any required runtime DLLs; audit imports, Authenticode sign helpers/app/installer; MSI or WiX package with sidecars in ffmpeg/bin.
- Linux: choose baseline glibc, package dependent shared libraries subject to platform policy, use $ORIGIN RPATH when needed, stage ffmpeg/bin beside editor; create AppImage or native package and verify on clean supported distributions.

The macOS compatibility staging entry point now invokes the Phase 8 local DMG workflow. The new scripts implement dependency checks, signing/notarization options and installer generation; execution and release acceptance remain pending. Release discovery never falls back to PATH.

Sources: [FFmpeg legal](https://ffmpeg.org/legal.html), [Slint licensing](https://slint.dev/pricing.html), [Slint FAQ](https://slint.dev/faqs).

## Phase 4 runtime capability additions

The normalization workflow requires FFV1, PCM s16le, MPEG-4 Part 2 and AAC encoders; Matroska/MP4 muxers; concat and source demuxers; `scale` with `reset_sar`, `pad`, `setsar`, `fps`, `tpad`, `trim`, `setpts`, `aresample`, `aformat`, `apad`, `atrim`, `asetpts` and `anullsrc` filters. The local verified runtime is FFmpeg/FFprobe 8.1.2. Vendor builds must explicitly verify these capabilities; older or stripped builds may reject a normalization command. No libx264/libx265 requirement was added. Copy mode also depends on a usable codec extradata hash from FFprobe. Lossless intermediates affect runtime disk space, not installer contents.

## Phase 5 audio runtime requirements

Verify the `ipod` (M4A), Matroska audio and WAV muxers, WAV `rf64=auto`, native AAC and PCM s16le encoders, `asetpts`, and the demuxers/decoders needed for supported sources. These are present in the local FFmpeg 8.1.2 build. Audio copy uses a conservative application allowlist; successful muxing of the actual stream is still required. No external/GPL audio encoder was added. Source tags are read for choosing tracks, while exported filenames retain source stream indices; comprehensive tag editing and metadata preservation are outside this phase.

## Phase 7 desktop resources

The development staging script `packaging/stage-dev-macos.sh` creates `dist/uFFmpeg Dev.app` with local FFmpeg symlinks and an ad-hoc signature. It is explicitly machine-dependent and must not be distributed. The release command remains separate and includes the app icon. `Info.plist` declares an arm64-compatible minimum macOS version of 11.0; the actual deployment floor must also be confirmed for every bundled binary in Phase 8.

Original SVG and generated ICNS/ICO/PNG assets are provided. Linux desktop-entry installation and Windows PE/installer icon embedding remain release packaging tasks. CI artifacts are development executables, not installers; no FFmpeg download or licensing decision is hidden in the CI workflow. See `docs/desktop.md` for platform dependencies and local build commands.
