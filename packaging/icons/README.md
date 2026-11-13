# App icon resources

Original source: `assets/app-icon.svg`, covered by the project's MIT license.

- `uffmpeg.icns`: macOS icon set (16–1024 px representations).
- `uffmpeg.ico`: PNG-backed 256 px Windows icon resource for Phase 8 packaging.
- `uffmpeg.png`: 256 px Linux desktop icon.

The PNG representations were rendered from the SVG with librsvg (`rsvg-convert`); macOS `iconutil` assembled the ICNS. Slint embeds the SVG as the window icon. Release platform integration is described in `docs/desktop.md` and `docs/packaging.md`.
