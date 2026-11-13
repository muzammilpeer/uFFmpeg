# Phase 8: Self-contained release

Status: Packaging implementation supplied; developer-run builds and release acceptance pending.

## Implemented

- Immutable upstream pins for FFmpeg 8.1.3, dav1d 1.5.4 and zlib 1.3.2.
- Native helper builds with static private dependencies, explicit codec/filter requirements, dependency/architecture audits, exact source archives and license/build records.
- Rust license/provenance collector, vendored source archives, package checksums and visible license-record access from Settings.
- Apple Silicon DMG command with local ad-hoc or explicit Developer ID signing/notarization modes.
- Windows x64 Inno installer command, icon embedding, static CRT request, DLL audit and optional payload/setup/uninstaller signing.
- Native Linux x64/ARM64 Debian package command, runtime dependency resolution and desktop/icon installation.
- Existing destinations are protected; no uploads/publication or credential provisioning occur automatically.

## Verification boundary

At the user's request, the agent did not run Phase 8 builds or generate release artifacts. Source formatting and Bash parsing passed; PowerShell, Inno, native compilers, signing and installer execution are unverified. No test cases were created or run. Phase 7 build results do not validate the later Phase 8 code changes.

## Commands and remaining work

Use [release commands](../packaging/release/README.md). Complete actual builds, license review, clean-machine installs without system FFmpeg, minimum-OS verification, signatures/notarization, upgrade/uninstall checks and source-rebuild review before declaring release readiness. Linux currently targets Debian-family packages; AppImage and other distributions are not claimed.

## GitHub release automation

Implemented three-platform tagged installer builds and direct draft Release uploads, version/checksum/asset checks, and maintainer publication instructions in `docs/github-release.md`. First hosted execution, signing setup and native release acceptance remain pending. No local release artifacts or test cases were generated.
