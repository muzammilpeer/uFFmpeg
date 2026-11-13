# Reviewed native helpers

Run `bash packaging/release/build-helpers.sh` on the target platform. It creates a target-triple directory with `bin/ffmpeg` and `bin/ffprobe` (`.exe` on Windows), exact `licenses/`, `source/` archives/configuration/build scripts, `build-info.txt`, `corresponding-source.txt` and `SHA256SUMS`.

Source pins live in `packaging/release/sources.env`. Private dav1d/zlib dependencies are static; architecture, external libraries, selected features and hashes are audited before promotion. Existing vendor output is never silently replaced. Move it aside before intentionally rebuilding.

Do not substitute the development Homebrew helpers. Native build completion is not license or release approval. Follow `packaging/release/README.md` for packaging and final acceptance.
