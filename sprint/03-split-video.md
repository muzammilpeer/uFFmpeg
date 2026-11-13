# Phase 3: Split video

Status: Implemented; macOS Apple Silicon build, lint and manual export/validation checks passed. Interactive UI review remains pending.

## Purpose and scope

Range model; in/out controls; multiple segments; stream-copy mode with keyframe limitations; accurate re-encode mode; safe destination selection.

## Work items

- Range model.
- In/out controls.
- Multiple segments.
- Stream-copy mode with keyframe limitations.
- Accurate re-encode mode.
- Safe destination selection.

## Completion evidence

Export selected ranges with preserved audio, reject invalid ranges and protect existing files.

## Dependencies

Phase 2 delivers the preceding foundation; later jobs may refine earlier workflows.

## Constraints

Rust and Slint only. No webview, React, JavaScript or Java. No test cases; use builds, diagnostics and manual inspection. No phase duration is committed.

## Diagram

![Project architecture](../assets/architecture.svg)

## Delivered behavior

Per-source validated range lists, decimal-second in/out fields, add/remove controls, accurate/copy choice and native destination-folder selection. Worker export keeps the UI event loop free; a busy guard prevents library/range edits and ordinary closing during operations. The CLI uses the same application service. Audio-only and cover-art-only files cannot enter the split workflow.

Accurate mode uses MP4 with native MPEG-4 Part 2 video and AAC audio, including all source audio tracks. Dimensions are padded to even values for yuv420p. Accuracy follows frame/sample boundaries, not arbitrary fractional instants; re-encoding can reduce quality. Fast mode copies the first moving video and all audio tracks into MKV and clearly discloses earlier-keyframe/duration differences. Subtitle, artwork, extra video and chapter handling is outside this phase. Native encoders avoid introducing a libx264/libx265 build requirement.

All ranges and destination names are preflighted. Export re-probes the source, stages each segment in the selected filesystem, checks for video, positive duration and retained audio-stream count, then publishes using no-overwrite persistence. Existing paths, including dangling symlinks, are rejected. If an output appears after preflight, publication still fails without overwriting it. This is a per-clip commit, not an atomic batch: earlier successes remain and their paths are reported when a later segment fails.

## Recorded manual evidence

- `cargo build --locked`, formatting, and Clippy for workspace libraries/binaries with warnings denied passed on Apple Silicon.
- Two ranges of 1.2 seconds exported from an Arabic/spaced path with two audio tracks. Accurate MP4 files probed at 1.200000 seconds with both AAC tracks; both decoded without errors.
- The same ranges exported via copy to MKV, retained both tracks and decoded successfully. Durations were 1.602 and 2.415 seconds due to copied keyframe preroll, matching the UI warning.
- A silent source exported successfully. A requested one-second off-frame range produced 0.96 seconds at 25 fps, illustrating frame-boundary rounding.
- Negative, reversed, zero-length, NaN and past-end ranges were rejected without outputs.
- An existing output remained byte-for-byte unchanged. A collision at segment 002 prevented segment 001 from being created, demonstrating full name preflight.
- Raw RGB that cannot be copied into MKV failed without publishing output or leaving staging files.
- Native app launch remained running. The desktop UI inspector does not list the running bare development executable, so picker, field interaction and full visual review remain pending. No test cases were added or run.

## Remaining limits

No playback/scrubbing, frame thumbnails at in/out points, export progress, cancellation, crash recovery, persistent ranges yet. Session stream selection was added in Phase 5. Normal closing waits for the worker; forced termination may leave a partial hidden staging file. Each segment has a one-hour command deadline. Windows/Linux and signed self-contained installers remain unverified. Lossy MPEG-4/AAC is the initial accurate preset; additional encoders/presets can be chosen after bundled-tool capability review.

Sources: [FFmpeg seeking and stream mapping](https://ffmpeg.org/ffmpeg.html), [no-overwrite temporary-file persistence](https://docs.rs/tempfile/latest/tempfile/struct.TempPath.html).

## Automatic batching extension

Whole-source fixed-length splitting is now available alongside saved manual ranges. See [the automatic splitting plan](05a-automatic-splitting.md) for workflow, safeguards and recorded evidence.

## Phase 6 integration

The shared FIFO queue now supplies progress, cancellation, retry, graceful shutdown and local job history for this workflow. Earlier worker/closing limitations above describe its original phase delivery; see [Phase 6](06-job-orchestration-and-resilience.md) for current behavior and forced-exit limits.
