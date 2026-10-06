# Changelog

All notable changes to anyview. The format follows Keep a Changelog; versions follow semantic
versioning, with pre-releases while the program is in beta.

## 0.1.0-beta.1 (2026-10-06)

The first beta.

### What it opens

- Images: PNG, JPEG, GIF, WebP, BMP, TIFF, ICO, TGA, QOI, SVG, JPEG XL, OpenEXR, Radiance HDR, PSD and
  ICNS, with animated pictures played by the clock. HEIC and AVIF through the `heif` plugin; camera raw
  files show their embedded preview, and are developed in full through the `raw` plugin.
- PDF documents, with page turning, zoom and printing.
- Text and source code with syntax highlighting, Markdown, CSV and TSV tables, spreadsheets (XLSX, ODS),
  JSON and JSONL as trees, and facts for office documents.
- EPUB books and CBZ comics, fonts (including WOFF), and archives (ZIP, tar, 7z and others) listed and
  read in place.
- Video and audio, played by your own mpv through the `mpv` plugin, with facts, pictures and
  conversion through the `ffmpeg` plugin.

### What it does

- One window for everything, and the same format library as the launcher's quick-look pane.
- Edits to pictures save in place with undo, redo, Revert To and Save a Copy; the original is kept first.
- Export and print for images, PDF and text; clips and audio extraction through the `ffmpeg` plugin.
- Follows the desktop's light and dark appearance, accent colour and reduced motion live.
- Opens files given as paths or `file://` URIs, forwards later opens to the running window through D-Bus,
  and stays warm for ten minutes after the last window closes.
- No codec is linked: playback, conversion, HEIC and raw development run your own tools as separate programs.

### Release basics

- `anyview --version` and `--help`; the plugins answer `--version` too.
- A panic prints a short report (thread, message, location, version) and keeps the last ten under
  `$XDG_STATE_HOME/anyview/crash/`.
- `THIRD-PARTY-NOTICES.md` for every crate built in, installed under `share/doc/anyview`.
- AppStream metainfo and a corrected desktop entry category.
- `cargo deny` passes in full, with each accepted advisory explained in `deny.toml`.
- quire, blitz-kit and the other repositories are pinned git dependencies: a fresh clone builds with
  `cargo build --locked`.
