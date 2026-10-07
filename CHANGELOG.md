# Changelog

All notable changes to anyview. The format follows Keep a Changelog; versions follow semantic
versioning, with pre-releases while the program is in beta.

## Unreleased

### Agents

- Viewer can be asked to open files by the desktop's agent layer (docket): `anyview.file.open` opens one or
  several files in the running window, or starts Viewer to show them. It only shows files; nothing an agent can
  ask edits or deletes one. A `files-viewer` skill tells the assistant how.

### Naming

- The program is called Viewer everywhere you see it: the welcome window, the helper sheet ("Viewer needs mpv to play videos."), the now-playing entry and the desktop entry. One constant, `anyview_core::APP_NAME`, holds the name; the binary, the app id and the bus names stay `anyview`, `org.quire.Anyview` and `org.quire.Anyview1`.

### Windows

- A window opens sized to its content, as Preview and QuickTime do: a picture with one image pixel to one
  screen pixel (on a 2x screen a 1200 by 800 picture opens in a 600 by 400 window), a video at its
  resolution, scaled down to fit when it is larger than 85% of the screen less the top bar (a 1920 by 1200
  screen is assumed before the screen is known) and never smaller than 480 by 320 (a small picture stays
  centred). The screen is the one the window is on, at its own scale, once the window is open. A PDF, and a picture the
  HEIC or RAW plugin decodes, take the window to their page or picture size as soon as they have loaded,
  unless you have already resized the window. Everything else opens at the usual size, and moving to the
  next file keeps the window as it is.
- The mouse wheel scrolls PDFs, pictures and text smoothly: each notch eases over a fraction of a second
  instead of jumping, and notches in a burst add up.
- quire v0.2.20: a touchpad scrolls the PDF, pictures and text with the fingers and carries on with the glide
  after a fast flick, as a native scroll does; Control with the wheel zooms one step a click.
- The playback bar thins itself to the window: in a narrow window it drops the export, the speed, the length, the
  volume and then the clock, in that order, and keeps play, the seek buttons and the progress bar. A PDF's
  and a picture's bars drop their least needed buttons the same way. The window's least size is still 480 by
  320, and what a narrow bar hides stays in the command palette and the right-click menu.

### Look

- quire v0.2.21: small icons are solid (filled) by default, a menu returns focus to what opened it, and the video controls' progress bar can be as narrow as 120 px, so the export button now shows on a window wide enough for the whole control bar (672 px and up).
- quire v0.2.19's Tahoe sizes: controls are 20, 24, 28, 32 and 40 tall (the capsule's buttons are the 32 Large,
  with an 18 px glyph), windows are rounder, and table and tree rows are quire's 32 px compact row.

### Platform

- What the platform cannot do is not offered. Open With, Share, Print, Open... and Show in Folder are left out
  of the palette, the right-click menu, the buttons on a card or a file that did not open, the welcome
  window and the keys (⌘O, ⌘P, ⌥⌘O, ⌘R) where the desktop service behind them is absent. The Install... of a
  missing tool is offered only where the system can install packages.
- The missing-tool sheet takes the package and program it names from the installer's own answer.

- The viewer's Linux desktop services are now an optional cargo feature, `quire-desktop` (on by default). A
  build with `--no-default-features` has no D-Bus code of its own: single instance goes through a per-user
  socket (latchkey), links and Show in Folder through the platform's opener, and Open With, Share, Print, the
  file chooser and the now-playing entry are absent and say so. Builds with the default features behave as
  before.

### Install

- `dist/install.sh` installs the `ffmpeg`, `heif` and `raw` plugins with the viewer, and builds the `mpv`
  plugin too: from `--with-mpv-from DIR`, `MPV_WGPU_DIR` or `../mpv`, else from mpv-wgpu's pinned revision,
  fetched with git into the cache. A plugin whose tool is not installed yet still installs and starts working
  once the package is there. If there is no mpv, git, network or build for mpv, only that plugin is skipped,
  with a one-line warning.
- With no mpv on the search path the mpv plugin still installs, naming `/usr/bin/mpv`, instead of being
  skipped: it works once mpv is installed (Install… offers it).
- `--without-plugin NAME` and `--no-plugins` replace `--with-plugin NAME`, which is accepted and ignored.

### Added

- Viewer offers to install a missing tool, as Totem offers a codec. A recording with no mpv, a HEIC with no
  libheif tools and a camera raw file with no LibRaw tools keep the "Needs" line and gain an Install… button
  on it; it asks "Viewer needs libheif tools to open HEIC photos.", and on Install the system's package
  service (PackageKit) installs the package and asks for the password itself. When it is done the file opens
  again in the same window with no restart, and a tool installed in a terminal is noticed the same way. Not
  Now closes the question and asks nothing more until the button is pressed again; where the system cannot
  install (a Flatpak sandbox, no PackageKit) or has no such package, the sheet says what to install.
  `dist/helpers/anyview.toml` names the packages for Fedora, Debian and Ubuntu, Arch and openSUSE and is
  installed to `share/quire/helpers/anyview.toml`.
- Audio plays without mpv. MP3, AAC (ADTS and M4A), ALAC, FLAC, WAV, AIFF and Ogg Vorbis are decoded in the
  viewer (pure Rust) and played through the sound card, with play and pause, seeking, volume, the resume
  position, the desktop's now-playing entry and media keys, and background playback. With the `mpv` plugin
  installed mpv still plays everything. Video and Opus audio still need mpv, and a machine with no sound
  output says so instead of playing.

### Changed

- The package budget of the viewer is 666 (the sound card library adds four packages and the missing-tool
  prompt two, ds-shell and ds-helpers; the launcher is unchanged). Building from source needs the ALSA development files (`libasound2-dev`, `alsa-lib-devel`).

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
