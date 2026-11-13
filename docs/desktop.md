# Desktop use and portability

## Navigation and smaller screens

The editor starts at 1140 × 800 logical pixels and permits resizing down to 960 × 600. The main workspace scrolls vertically so export controls remain reachable on smaller displays. Detail, source and job lists scroll independently. Native widgets use a consistent dark palette, brighter supporting text, and named time/segment fields. Standard buttons and checkboxes expose their labels through Slint accessibility. These changes are an accessibility foundation, not a completed screen-reader audit.

The File and View menus expose keyboard commands. Use Command on macOS and Control on Windows/Linux:

| Shortcut | Action |
| --- | --- |
| Command/Control + O | Add media through a native picker |
| Command/Control + 1 | Split video |
| Command/Control + 2 | Merge videos |
| Command/Control + 3 | Extract audio |
| Command/Control + 4 | Jobs |
| Command/Control + comma | Settings |
| Tab / Shift+Tab | Forward/backward widget focus |
| Space | Activate a focused button or checkbox |

Menus route the same callbacks as visible controls. Export remains an explicit button action; editing a text field does not submit an export. Current session media and ranges are not persisted as a project.

## Preferences and folders

Choose the desired accurate/copy mode, audio format and automatic segment length/unit in the editing views, then choose **Settings → Save current choices as defaults**. These choices apply at the next startup. Import and output folders are remembered separately as soon as a picker choice is accepted, even if a subsequent media operation fails. Merge remembers the selected output file's parent directory. Missing folders fall back to the native picker default.

Preferences are versioned JSON in `preferences.json`, next to the local `jobs.json` history. The same `UFFMPEG_STATE_DIR` override applies to both. Default paths are macOS `~/Library/Application Support/uFFmpeg`, Windows `%LOCALAPPDATA%/uFFmpeg`, and Linux `$XDG_DATA_HOME/uFFmpeg` or `~/.local/share/uFFmpeg`. Saved folder names are local data; no upload or telemetry is added.

Writes use a temporary file, flush, and atomic replacement. Malformed, oversized or unsupported-version settings are preserved and a warning appears in Settings; defaults are used for the session. To recover such a file, close the app and move it aside before reopening. Saving refuses to silently replace malformed settings. No global operating-system preferences are changed.

## Finding exports

Select a job in **Jobs**, then choose **Show output in folder** (also in File). The button becomes available once that job has published any output, including a partially completed or cancelled batch. Finder and Explorer select the first published file; Linux opens its containing folder through `xdg-open`. Remaining filenames are in job details. A removed/unavailable file reports an error. File-manager launch happens off the UI thread and uses argument-based process invocation, without a shell.

## Local macOS bundle

Run `packaging/stage-dev-macos.sh`, then open `dist/uFFmpeg Dev.app`. The script builds the locked debug workspace, adds the app icon and metadata, links this machine's FFmpeg/FFprobe into its resource layout, and ad-hoc signs the app. It does not recursively sign or alter Homebrew helpers. The executable is replaced through a new file rather than overwritten in place. Finish work and close the development app before rebuilding to ensure the next launch uses the new version.

This development app depends on the local helper installation and its libraries. It is not the self-contained final installer and must not be redistributed. The separate `stage-macos.sh` release path still requires reviewed helper binaries, notices and build/source records; Phase 8 handles dependency audits, distribution signing and installers.

## Platform builds

`.github/workflows/desktop.yml` describes native macOS, Windows and Linux jobs: formatting, strict library/binary Clippy and locked builds, without test cases. It uploads development executables only. No workflow has been dispatched from this local folder.

`packaging/Dockerfile.linux` supplies a Debian Bookworm build environment. Build with `docker build -f packaging/Dockerfile.linux -t uffmpeg-linux .`; use `--platform linux/amd64` on both build and run for x64 on Apple Silicon. Mount source read-only and use a distinct target volume per architecture. The installed Debian FFmpeg is a development tool; its older version does not establish compatibility with every export mode, especially normalization's `scale=reset_sar` requirement.

Linux desktop sessions require X11/Wayland graphics libraries, fonts, `xdg-open`, and a working XDG desktop portal/backend for native dialogs. A container compilation or CLI run does not verify those interactive integrations. Windows needs a native MSVC toolchain for the CI build; the local GNU-target check validates conditional Rust/Slint code without producing a linked Windows executable.

Original icon source is `assets/app-icon.svg` (project MIT license); generated ICNS, ICO and PNG resources live in `packaging/icons`. macOS staging and Slint's window icon use these assets. The Windows installer/executable resource integration and Linux installation of `uffmpeg.desktop`/icons are Phase 8 packaging steps. The desktop entry expects an installed `uffmpeg` launcher; development CI binaries retain the internal `frameforge-desktop` name.

References used for build configuration: [Slint build prerequisites](https://github.com/slint-ui/slint/blob/master/docs/building.md), [GitHub runner selection](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job). The pinned Slint 1.16.0 local widget/API sources were used for menus, keyboard modifiers and accessibility properties.

If Docker's internal disk is constrained, bind a project-local `work/` cache directory to `/build` instead of allocating build volumes there. Keep ARM64 and x64 caches separate. Source remains mounted read-only. The Phase 7 local verification used this arrangement after reclaiming only its own disposable Docker build caches; no unrelated Docker data was pruned.

The container default builds the desktop and its workspace dependencies serially with incremental compilation disabled and GNU ld’s `--no-keep-memory` option, to accommodate small Docker memory allocations. Run only one architecture build at a time on a 2 GB Docker VM. Native CI retains standard workspace builds.
