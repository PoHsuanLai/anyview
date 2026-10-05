# anyview

anyview is a cross-platform, all-in-one file viewer for a macOS-style desktop: images, PDF, text
and code, Markdown, tables, audio and video, fonts, archives and books (EPUB and CBZ) in one window, and the same
format library as the launcher's preview pane. It is a Rust workspace written in layers, from the
pure vocabulary (`anyview-core`: what a file is, how it is sniffed, the units, actions, edits,
exports and view memory) up to the app, and builds on the design system in the sibling quire
checkout. `ARCHITECTURE.md` is the map, `CONVENTIONS.md` the rules, `FINDINGS.md` the open items.

## Playing and converting recordings

anyview links no codec. Video and audio are played by your own `mpv` and read and converted by your own
FFmpeg, through two small plugins that run them as separate programs, so the codecs you get are the ones
your distribution gives you. Install the runtime packages `mpv` and `ffmpeg` (`ffprobe` comes with it), then
the plugins:

```sh
dist/install.sh --with-plugin mpv --with-plugin ffmpeg      # [--mpv /path/to/mpv] [--prefix DIR]
```

The mpv plugin needs mpv-wgpu's C plugin built from a checkout (`MPV_WGPU_DIR`, default `../mpv`). Without the
plugins a recording opens as its facts with the package that would play it named, and the export sheet says
which package adds the recording formats. `dist/uninstall.sh` removes everything the installer wrote.

## HEIC and camera raw files

HEIC/HEIF (and AVIF in a build without its own decoder) and the full development of camera raw files go through
two more plugins that run your own tools: `anyview-heif` (libheif's `heif-dec` or `heif-convert`; Fedora
`libheif-tools`, Debian `libheif-examples`) and `anyview-raw` (LibRaw's `dcraw_emu` or `dcraw`; Fedora
`LibRaw-samples`, Debian `libraw-bin`).

```sh
dist/install.sh --with-plugin heif --with-plugin raw
```

A camera raw file shows its embedded JPEG preview without any plugin (CR2, NEF, ARW, DNG, ORF, RW2 and the other
TIFF-based formats, and CR3), with a `Needs: anyview-raw` row offering the full-quality development. A HEIC without
the plugin opens as its facts with `Needs: anyview-heif`.
