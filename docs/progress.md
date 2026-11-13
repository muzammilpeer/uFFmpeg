# Phases 1–6 progress — 28 October 2026

Project directory: /Users/muzammilpeer/Git/uFFmpeg. The assigned directory was inspected and empty before files were placed here. Earlier generated-folder scaffolding was relocated here.

## Delivered

Eight phase plans in sprint/, original architecture and roadmap SVG assets, four-module Cargo workspace and dependency lockfile, compiled Slint shell with accessible AboutSlint attribution, worker-based startup diagnostics, paired native helper discovery and typed FFprobe JSON metadata. Packaging policy covers self-contained installers, build provenance, dependency audits and license/source obligations. Unsigned macOS staging skeleton refuses missing vendor release inputs.

## Verification

- cargo check passed for the initial workspace.
- cargo build --locked passed in this assigned directory on arm64 macOS with Rust 1.96.1.
- cargo fmt --all -- --check passed.
- Native diagnostics discovered Homebrew FFmpeg and FFprobe 8.1.2 for development.
- Manual probe of a generated one-second MP4 returned MPEG-4 video at 160×90, AAC audio and duration 1.000000.
- macOS Info.plist validation and staging script syntax checks passed.
- Staging correctly refused absent reviewed vendor FFmpeg binaries. This is an expected release guard, not an installer success.
- Native shell launch was checked for early exit; this is not a full visual or accessibility review.

No test cases were written or run.

## Remaining

Phase 7 desktop refinement and portability is next. Split, merge, audio extraction and per-source split/audio stream choices are implemented. UI visual inspection is still needed. Windows/Linux and the declared minimum Rust version have not been verified. Release FFmpeg binaries, source/license artifacts, shared-library auditing, signing, notarization and actual installers are not supplied. The app is currently a development binary using local media tools, not a self-contained release.

The original assistant architecture was unavailable as content in the referenced chat; phase titles were reconstructed from confirmed requirements and this limitation is explicit in sprint/README.md.

## Phase 2 — 28 October 2026

Delivered native multi-file import, background validation/inspection, session media list with selection/reorder/removal, metadata view, temporary FFmpeg thumbnails, duplicate path skips, per-file failure messages, audio-only handling and preview warnings. Added a CLI `--inspect PATH` entry point for the shared application import service. Native calls now use deadlines and bounded file-backed capture. Cargo.lock includes the pinned rfd 0.17.2 native dialog dependency.

Verification: locked Apple Silicon build, formatting check and Clippy for workspace libraries/binaries with warnings denied passed. Manual service inspection succeeded for an Arabic filename containing spaces (MPEG-4 video, AAC audio, thumbnail) and a spaced audio-only WAV path (PCM metadata, no thumbnail). Unsupported text, empty media, a directory and a missing path returned failure with actionable messages. Native desktop startup remained alive for four seconds. Interactive UI automation timed out opening the development app, so file-picker behavior, reorder/remove/duplicate flows and visual layout are not claimed as manually verified. Windows/Linux builds remain pending. No test cases were added.

## Phase 3 — 28 October 2026

Implemented multi-range split controls and shared CLI/application export, accurate MPEG-4/AAC MP4 and keyframe-limited copy MKV modes, retention of all audio tracks, source-duration validation, native folder selection, complete destination preflight, staged output validation and no-overwrite publication. Export runs off the UI thread; ordinary closing and library edits wait for completion. No extra dependency or GPL encoder was introduced.

Locked build, formatting and strict library/binary Clippy checks passed. Manual exports from Unicode/spaced paths preserved two audio tracks and decoded without errors. Accurate 1.2-second selections produced 1.2-second clips; copy durations differed with keyframe preroll as documented. Silent video exported. Invalid ranges and collisions were rejected, including a collision in a later batch name. A deliberately incompatible raw-RGB copy export failed cleanly with no published or staging files. No test cases were written or run.

The bare native development executable launched but was absent from the UI inspector's app inventory, so interactive controls and full visual layout remain unverified. Platform and installer release gates are unchanged. Cancellation/progress and forced-exit cleanup remain Phase 6 work. See sprint/03-split-video.md for detailed scope and evidence.

## Phase 4 — 28 October 2026

Implemented whole-clip merging in library order, visible stream compatibility diagnostics and an explicit normalization choice. Fast copy uses concat-demuxer MKV with conservative stream/configuration/timebase/layout checks. Normalization fits the first clip's display canvas, uses 30 fps video and 48 kHz stereo audio, preserves track positions and fills missing audio tracks with silence. Lossless FFV1/PCM intermediates avoid an additional lossy generation before final MPEG-4/AAC MP4 output. Native save selection, background execution, source re-probing, output collision/extension checks, staged stream/duration validation and no-overwrite publication are shared with CLI entry points.

Locked build, formatting and strict library/binary lint checks passed. Manual merges verified forward/reversed visual order, two retained audio tracks, mixed-resolution/frame-rate/audio-layout normalization, silent sections and all-silent media. Normalized outputs decoded cleanly and a delayed-video source retained its audio/video timing; fast-copy AAC showed small join timestamp overlaps through the null output muxer, which are now disclosed in the product UI and documentation. Unicode/space/apostrophe/backslash paths worked in copy mode; normalization also handled newline-containing input/output paths. Incompatible copy, insufficient/audio-only input lists, wrong extensions and existing outputs were refused. Protected file hashes remained unchanged and ordinary operations left no temporary merge artifacts.

Native startup passed; interactive UI review remains pending. No test cases were added or run. Signed self-contained distribution and Windows/Linux verification are still pending. See sprint/04-merge-videos.md for details.

## Phase 5 — 28 October 2026

Implemented per-source video/audio selections, explicit selected-stream split mappings and an audio extraction view/CLI for full media or saved ranges. Outputs are one audio track per file: approved-codec copy to MKA, explicit native AAC to M4A or signed 16-bit PCM WAV. Audio-only sources use the shared range editor. Labels expose actual stream IDs and available language/title metadata. Merge continues its whole-source stream policy, and the import preview remains clearly identified as the first video stream.

Locked build, formatting and strict library/binary lint passed. Manual media checks confirmed two selected audio tracks, full-copy packet hashes identical to source, exact 1.2-second AAC/WAV ranges, expected PCM bit depth, audio-only input, selected second-video/one-audio splits and silent video splits. A copy-range preroll issue was corrected and rechecked at 1.195 seconds for a 1.2-second request. Missing/wrong/duplicate selections, absent audio, unsupported copy codecs, bad ranges and output collisions were refused. Protected output hashes stayed unchanged; a collision in the last planned track/range prevented any earlier export. Explicit WAV conversion succeeded after mu-law copy rejection, with no silent fallback or leftover staging files.

Native startup passed; interactive UI review remains pending. Queue/progress/cancellation, crash recovery, Windows/Linux validation and self-contained signed release packaging remain later work. No test cases were added or run.

## Automatic fixed-length splitting — 28 October 2026

Added Auto split entire source to both video and audio views, seconds/minutes input (2-minute default), live segment/file count and final-length preview, and direct output-folder export. Manual ranges remain intact and automatic exports honor selected streams/formats. Shared domain planning avoids cumulative decimal drift; workers re-probe source duration and reuse safe export services. Length/count limits, a video-frame minimum, sub-frame tail absorption and batch name preflight protect quick operations. CLI automatic-video, automatic-audio and plan-preview entry points are included.

Locked build, formatting, strict lint and manual service checks passed. A real 251-second video produced 120/120/11-second clips; audio batching produced 1.5/1.5/1.0-second clips for each selected track. Exact multiples and longer-than-source intervals behaved correctly. Invalid and pathological lengths/output counts are rejected before processing, including sub-frame video lengths corrected after edge inspection. A late output collision left the existing file unchanged and created no earlier clips. Interactive UI review remains pending; queue/cancellation/progress are still Phase 6. No test cases were added.

## Phase 6 — 28 October 2026

Implemented one FIFO job worker for all desktop operations, immutable input/settings snapshots, queued/running cancellation, retry, real FFmpeg progress and per-job results. Replaced feature-specific worker/timer pairs with a shared queue view/controller. Library preparation remains available while work runs. Engine processes use bounded stdout/stderr/progress readers, deadlines, cancellation and kill/wait guards; staged publication is gated against cancellation so completed files remain while unfinished files are removed. Normal closing cancels and waits for cleanup.

Added local asynchronous atomic job history, a single-writer lock, Interrupted recovery without automatic replay, warning/fallback for unavailable history and a CLI typed batch workflow with stdin cancel/retry/quit. Normal shutdown joins the operation worker and flushes/joins the metadata writer. History and source/output paths are bounded and stored locally. No new external runtime dependency was added; existing serde dependencies are now used directly in application/desktop manifests.

Locked build, formatting and strict lint passed. Manual checks verified real progress, FIFO execution, queued/active cancellation, failure isolation, retry after source repair, partial-batch retention, native-child exit on shutdown, artifact cleanup, history locking, safe Interrupted recovery, malformed-history preservation and final asynchronous history flush. Automatic video/audio and normalized merge still exported successfully. Native startup passed with project-local history. Interactive UI/close-window QA, Windows/Linux and self-contained signed packaging remain pending; forced termination has the explicit recovery limits recorded in Phase 6. No test cases were added or run.

## Phase 7 — 29 October 2026

Delivered native File/View menus and Command/Control shortcuts, smaller resizable/scrollable desktop layout, dark native-widget palette, brighter supporting text and accessible time/segment/preview/progress labels. Job rows now update only when their state/list changes, preserving keyboard focus during progress updates. Added explicit saved export defaults, separately remembered import/output folders, bounded/versioned preferences with atomic persistence and malformed-file preservation. Jobs can reveal their first published output using the platform file manager, including partial batches. Windows path handling converts canonical extended drive/UNC paths for Explorer.

Added an original SVG app icon and generated ICNS/ICO/PNG assets, Linux desktop entry, macOS icon/deployment metadata, and an ad-hoc-signed local development app staging script. Development helpers are links to the installed FFmpeg pair, explicitly not a self-contained distributable. The reviewed release-helper contract and Phase 8 obligations remain unchanged. Added native three-platform CI configuration and an isolated Linux build container. No CI run or release installer is claimed.

Verification so far: locked macOS Apple Silicon build and strict Clippy passed; Windows x64 GNU-target workspace check and strict Clippy passed (no Windows linker/runtime). Linux ARM64 Debian container build passed, and its executable discovered FFmpeg/FFprobe and planned the expected 120/120/11-second segments for a 251-second source. Linux x64 also completed successfully under emulation and ran bundled-engine diagnostics and the 120/120/11-second segment plan. Final Linux ARM64 and x64 executables were saved before the user requested command-only artifact generation. Docker disk/memory limits were resolved using project-local caches and serial low-memory linking. Formatting passed. No test cases were added or run.

Native macOS inspection now works through a local app bundle. Confirmed the rendered split/settings layout, named accessible controls, Tab focus, saving default preferences, resource-based helper discovery, and native-menu Command-comma/Command-O behavior. An initial focus-scope shortcut implementation was replaced with native menu shortcuts after inspection exposed its failure. A separate preview with project-local state was used when the user's editor was active; further picker/export/reveal/restart interaction was not claimed because concurrent user activity continued in the preview too. User media sessions and outputs were retained. Final small accessibility/focus changes were compile/lint checked but not re-inspected interactively.

Remaining acceptance: linked Windows build and runtime; native Windows/Linux dialogs, output reveal, resizing/high DPI, keyboard and screen-reader inspection; full macOS export/reveal/default-restoration flow. These are explicit Phase 7 acceptance gates, not blockers to continuing Phase 8 implementation. Reviewed self-contained FFmpeg builds, notices/source records, dependency audits, release signing/notarization and installers are Phase 8.

## Phase 8 implementation — 29 October 2026

After the user requested command-only artifact generation, implemented release build/packaging source without running those workflows. Added pinned FFmpeg/dav1d/zlib helper builds, static private dependencies, capability/architecture/dependency checks, source/license/hash records, a Rust dependency-license collector, vendored project-source archives, macOS DMG/sign/notarize commands, Windows Inno/CRT/icon/signing integration, Linux Debian packaging and Settings access to installed license records. Upstream immutable tag revisions were read directly; no helper source archives or release outputs were downloaded/generated for Phase 8.

Validation was limited to Rust formatting/parsing and Bash syntax. PowerShell/Inno were reviewed as source but are not available here for parsing/execution. No Phase 8 compilation, installer, signing, notarization, clean-machine check or test case was run. Existing Phase 7 development outputs remain, and the already-running Linux x64 build had completed before the stop attempt. See packaging/release/README.md for the exact user-run commands and unresolved release acceptance gates.

## GitHub release preparation — 29 October 2026

Added separate three-platform branch/PR checks and tag-triggered installer CI. Version tags must match both release version declarations. Native helper builds, strict lint, packaging, checksums and exact release asset names are checked; installers/source archives upload directly to a draft prerelease with the built-in GitHub token. All platforms must finish before the draft is marked ready for maintainer review; publishing remains manual. No temporary Actions binary artifacts are retained. The initial CI channel is unsigned/ad-hoc signed, with standard macOS ARM64, Windows x64 and Ubuntu 24.04 x64 runners.

Found missing upstream license texts in packaged Rust dependencies and added exact-revision collection with provenance and fail-closed handling. Source bundles now include their supporting scripts and all platform packaging resource files. Added Windows checkout line-ending rules and credential/build-output exclusions. GitHub setup, tagging, asset attachment, reruns, quotas and publication instructions are in `docs/github-release.md`.

Local validation: Rust formatting, Bash/Python/YAML syntax and read-only Cargo metadata inspection. No release binaries/installers were generated locally, and no new test cases were added. Full compilation, installer creation and native acceptance must still run in CI/on clean machines. GitHub publication has not been performed.

The user's completed Apple Silicon helper build initially failed because its version string was `n8.1.3`. Fixed exact version parsing in Bash and PowerShell, added a revision-checked `--resume` path, and successfully audited/promoted the existing helpers into `packaging/vendor/aarch64-apple-darwin` without recompilation. Architecture, dynamic dependencies, required encoders/decoders/muxers/filters and helper/source checksums passed. Initialized local Git on `main`; no remote or commit has been created. Action implementations are pinned to verified upstream commit IDs.
