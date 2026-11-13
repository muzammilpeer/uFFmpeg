# Phase 7: Desktop refinement and portability

Status: Implementation delivered; platform interaction/release gates remain open.

## Delivered

- Native File/View menus with platform keyboard shortcuts, standard widget focus navigation and named time/segment fields.
- Smaller resizable window, vertical overflow scrolling, dark widget palette and improved supporting-text contrast.
- Explicit saved startup defaults for split mode, audio format and segment interval; separately remembered native import/output folders.
- Versioned, validated and atomically saved preferences beside job history, with malformed-file preservation and visible warnings.
- Job output reveal through Finder, Explorer or the Linux file manager, including published files from partial batches.
- Original vector app icon plus ICNS/ICO/PNG resources, macOS metadata and a Linux desktop entry.
- Locally runnable, ad-hoc-signed macOS development bundle; release staging retains reviewed-helper requirements.
- Native macOS/Windows/Linux CI build configuration and reproducible Linux container build environment. No test cases.

## Verification and remaining gates

See the dated Phase 7 entry in `docs/progress.md` for actual build and interaction evidence. A Windows target check is not a linked build, and a Linux container run is not a native desktop inspection. CI configuration is not evidence that CI has run.

Still required before claiming complete platform acceptance: Windows linked build/runtime, native Windows/Linux picker, reveal, keyboard/focus and scaling checks, VoiceOver/Narrator/Orca review, and smaller-screen/high-DPI inspection across supported desktops. Full native UI automation was interrupted by concurrent user interaction; existing media sessions were retained.

Phase 8 supplies reviewed self-contained FFmpeg binaries, dependency/license audits, signing/notarization and installers. Development Homebrew links and container FFmpeg are not release payloads.

## Usage and implementation notes

See [desktop guide](../docs/desktop.md), [packaging policy](../docs/packaging.md) and [progress](../docs/progress.md).

![App icon](../assets/app-icon.svg)
