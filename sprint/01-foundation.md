# Phase 1: Foundation

Status: Implemented; macOS build and manual engine checks passed; release tooling remains gated.

## Purpose and scope

Cargo workspace; native Slint shell; domain, application and engine boundaries; paired FFmpeg/FFprobe discovery; metadata probe; bundling contract.

## Work items

- Cargo workspace.
- Native slint shell.
- Domain, application and engine boundaries.
- Paired ffmpeg/ffprobe discovery.
- Metadata probe.
- Bundling contract.

## Completion evidence

Build the Apple Silicon shell, run diagnostics and inspect real media metadata.

## Dependencies

None; establishes the workspace.

## Constraints

Rust and Slint only. No webview, React, JavaScript or Java. No test cases; use builds, diagnostics and manual inspection. No phase duration is committed.

## Diagram

![Project architecture](../assets/architecture.svg)
