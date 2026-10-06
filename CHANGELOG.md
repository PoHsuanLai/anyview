# Changelog

All notable changes to anyview. The format follows Keep a Changelog; versions follow semantic
versioning, with pre-releases while the program is in beta.

## Unreleased

### Install

- `dist/install.sh` installs the `ffmpeg`, `heif` and `raw` plugins with the viewer, and builds the `mpv`
  plugin too: from `--with-mpv-from DIR`, `MPV_WGPU_DIR` or `../mpv`, else from mpv-wgpu's pinned revision,
  fetched with git into the cache. A plugin whose tool is not installed yet still installs and starts working
  once the package is there. If there is no mpv, git, network or build for mpv, only that plugin is skipped,
  with a one-line warning.
- `--without-plugin NAME` and `--no-plugins` replace `--with-plugin NAME`, which is accepted and ignored.

### Added

- Audio plays without mpv. MP3, AAC (ADTS and M4A), ALAC, FLAC, WAV, AIFF and Ogg Vorbis are decoded in the
  viewer (pure Rust) and played through the sound card, with play and pause, seeking, volume, the resume
  position, the desktop's now-playing entry and media keys, and background playback. With the `mpv` plugin
  installed mpv still plays everything. Video and Opus audio still need mpv, and a machine with no sound
  output says so instead of playing.

### Changed

- The package budget of the viewer is 664 (the sound card library adds four packages; the launcher is
  unchanged). Building from source needs the ALSA development files (`libasound2-dev`, `alsa-lib-devel`).

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
- No codec library is linked: playback of video, conversion, HEIC and raw development run your own tools as separate programs.

### Release basics

- `anyview --version` and `--help`; the plugins answer `--version` too.
- A panic prints a short report (thread, message, location, version) and keeps the last ten under
  `$XDG_STATE_HOME/anyview/crash/`.
- `THIRD-PARTY-NOTICES.md` for every crate built in, installed under `share/doc/anyview`.
- AppStream metainfo and a corrected desktop entry category.
- `cargo deny` passes in full, with each accepted advisory explained in `deny.toml`.
- quire, blitz-kit and the other repositories are pinned git dependencies: a fresh clone builds with
  `cargo build --locked`.
