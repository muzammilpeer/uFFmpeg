# Feature extension: automatic whole-source splitting

Status: Implemented as an extension to Phases 3 and 5. Phase 6 has not started.

## User workflow

Import/select a video or audio source, enable Auto split entire source, enter a length in seconds/minutes, review the computed count, then choose Split whole video/audio and an output folder. The default is 2 minutes. Current stream selections and format settings apply. A shorter final segment is retained. This is one selected source split into many files; multi-source queueing is separate Phase 6 work.

## Implementation

A shared domain planner partitions the entire duration using microsecond ticks rather than cumulative floating-point addition. Exact multiples produce no empty last range; longer-than-source intervals produce one whole-source range. Video planning validates the selected stream and, when frame timing is known, rejects sub-frame interval sizes and absorbs a sub-frame final tail into the preceding clip. Audio uses the mathematical intervals, creating one output per selected track per interval.

The desktop and CLI share interval parsing and planning. Live preview is local metadata work; workers re-probe the source at export and reuse the existing staged/no-overwrite video/audio services. Manual ranges are never modified. The shared panel is scrollable so automatic controls and manual controls occupy the same space rather than growing the window indefinitely.

Input guards reject zero, negative, non-finite or sub-millisecond lengths, unknown/nonpositive duration and more than 10,000 automatic outputs (audio track multiplication included). Copy modes retain their keyframe/packet limits. Very short or irregular source timing can still prevent an export; reports retain earlier successes if a later clip fails. Existing paths and all later batch names are preflighted before processing. Source files remain unchanged.

## Manual evidence

- Locked Apple Silicon build, formatting and strict library/binary Clippy checks passed. No test cases were added.
- A real 251-second source split at 2 minutes produced 120.000000-, 120.000000- and 11.000000-second video clips, retaining audio; all decoded successfully.
- A 4-second source split at 2 seconds produced exactly two video clips. A 0.3-second mathematical plan at 0.1 seconds produced three ranges with no phantom extra range.
- Two audio tracks split at 1.5 seconds produced six WAV files: 1.5, 1.5 and 1.0 seconds per track, all decoding cleanly.
- An interval longer than the source produced one clip while honoring an explicitly selected second video/audio stream. Audio-only input is also supported.
- A collision at a later segment stopped the batch before the first output; the protected file hash was unchanged.
- Zero, negative and non-finite lengths were rejected. Edge inspection exposed unusable sub-frame video batches; a preflight frame-duration guard and tail handling were added, then rechecked. Oversized mathematical/video/audio output plans are refused before any output processing.

## Remaining verification and limits

Native startup passed a four-second check. Interactive native controls and full visual layout require review; build/service checks do not claim that review. Windows/Linux and signed bundled distribution remain unverified. Queue/progress/cancel and crash recovery remain Phase 6. Copy export is approximate at media boundaries, and fixed-length planning does not promise gapless compressed audio or frame-independent video cuts.

## Phase 6 integration

The shared FIFO queue now supplies progress, cancellation, retry, graceful shutdown and local job history for this workflow. Earlier worker/closing limitations above describe its original phase delivery; see [Phase 6](06-job-orchestration-and-resilience.md) for current behavior and forced-exit limits.
