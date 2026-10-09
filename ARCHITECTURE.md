# Architecture

anyview is the viewer: one program that opens every common file type (images, PDF, text and
code, Markdown, tables, audio and video, fonts, archives, books, office documents) and the format
library the launcher shares, so a file looks the same in the launcher's pane and in a window. This
file is the map: which crate owns what, what may depend on what, where each concept lives, the
exact shape of each trait, and the commands that gate a change. `CONVENTIONS.md` holds the rules
every repo shares; where it and this file disagree, this file wins for anyview.

anyview is a sibling checkout of quire (`../quire`) and takes its design-system crates by path,
the way sill does (quire `CONSUMING.md` section 1). The pinned block of `[workspace.dependencies]`
in `Cargo.toml` is copied verbatim from quire's `docs/workspace-deps.toml`; a new dependency is a
change to that file first.

The plugin machinery that does not depend on what a capability is (frames, the manifest envelope, discovery,
ranking, starting and containing a process) is bayonet, a repo of its own
(`github.com/PoHsuanLai/bayonet`), taken through one line of `[workspace.dependencies]` (section 2l).

## 1. Crates and allowed edges

Layers go lowest first. A crate names only crates in a lower layer, and
`scripts/check-boundary.sh` enforces the edges and the external boundaries below. A crate marked
planned has no directory yet; its row is the rule it will carry.

| Layer | Crate | Status | Purpose |
| --- | --- | --- | --- |
| L0 | `anyview-core` | exists | pure vocabulary: kinds, sniffing, units, sequence, actions, edits, exports, view memory, the `Peek` trait |
| L0 | `anyview-plugin-protocol` | exists | the plugin protocol, version 1: its messages, over bayonet's length-prefixed JSON frames, which it re-exports; the one crate a plugin author depends on |
| L1 | `anyview-store` | exists | the recently-viewed history and per-file view memory on disk: one format, a read API (sill reads it) and a write API; and the save pipeline (`Pending` to `BackedUp` to `Written`) with the versions store of kept originals under `<state>/anyview/versions` |
| L1 | `anyview-image` | exists | raster and vector images: decode to upright RGBA8, a downscaled peek with EXIF facts, encode for export, lossless JPEG rotation |
| L1 | `anyview-pdf` | exists | pdfrum: open and share a document, lay out pages, plan and draw tiles, search across the document, outline, links, page edits, exports |
| L1 | `anyview-media` | exists | video and audio: the typestate player session over the person's own mpv, run as a child process, and the driver an actor runs (feature `player`); the built-in audio player for a machine with no mpv (feature `audio`: symphonia decodes, cpal plays); and the plan, the names and the asks of the exports a plugin writes; links no libmpv and no libav |
| L1 | `anyview-text` | exists | text: encodings and windowed lines, code highlighting into token classes, Markdown to HTML, CSV tables, spreadsheets (calamine), JSON trees, and the five text peeks |
| L1 | `anyview-archive` | exists | archives: zip, tar, 7z and compressed-stream listings read inside a byte budget, extracting one entry, the archive peek, and an office package's title, author, count and embedded picture |
| L1 | `anyview-book` | exists | books: an EPUB's package (metadata, reading order, contents), a chapter as sealed HTML (allowlisted markup, sealed styles, images inlined as `data:` URLs), a comic zip's pages in natural order, and the covers of both; reads the zip through `anyview-archive` |
| L1 | `anyview-font` | exists | fonts: names and glyph count read with skrifa, the specimen as vector outlines, and the font peek |
| L1 | `anyview-plugin` | exists | plugins as values: the viewer's capabilities and what each handles (the `[[provides]]` entries of bayonet's manifest), the registry of which plugin serves a kind and capability, the package to suggest when none does |
| L1 | `anyview-fs` | exists | the one place a path is opened: `OnDisk` makes the `Input` of a `FilePath` or a `Source`, `OpenFile` and `open_regular` open a regular file (a FIFO, a device or a socket is refused); it depends on `anyview-core` alone and builds on every platform the headless peek does |
| L2 | `anyview-export` | exists | the exports and printouts of images, PDFs and text documents: runs the jobs each format plans, writes each file beside the original through a temporary file renamed into place, and makes the PDF a printer takes |
| L2 | `anyview-platform` | exists | the edge: traits, their Linux implementations and fakes |
| L3 | `anyview-peek` | exists | the light tier: the registry that maps every kind to its `Peek`, the PDF, folder, video and audio (pure-Rust header parsers) and facts-only peeks, the type-erased `AnyPeeked`, `look` (its one front door: probe a file and peek at it as `Peeking` says), and the pane view (what the launcher links) |
| L4 | `anyview-ui` | exists | the viewer: its pure machines (chrome, panel, palette, context menu, sheet, navigation, presentation, loading, the five stages, key routing and the root that composes them), the blocking work a worker does for it (`io`), one Dioxus view per family of formats (`families`: images, text, PDF pages (which also show books, bound as PDFs) and the facts view) and the window that draws every region (`views`) |
| plugin | `anyview-ffmpeg` (in `plugins/`) | exists | the FFmpeg plugin: a program that speaks protocol v1 and runs the person's `ffprobe` and `ffmpeg` for facts, pictures and exports of video and audio; links no libav (section 2l) |
| plugin | `anyview-heif`, `anyview-raw` (in `plugins/`) | exists | the picture plugins: programs that speak protocol v1 and run the person's libheif tools (HEIC, HEIF, AVIF) or LibRaw's `dcraw_emu`/`dcraw` (a raw file in full, its preview as a thumbnail); they link no libheif and no LibRaw (section 2l, "The picture plugins") |
| plugin kit | `anyview-tool-kit` (in `plugins/`) | exists | what the two picture plugins share: finding a tool (manifest argument, environment variable, search path), running it with a deadline and a cancel, reading the PNG, TIFF or PPM it wrote, and the protocol's request loop |
| dev | `anyview-plugin-fake` | exists | a test plugin that speaks protocol v1 for one invented kind, and the integration tests of discovery and the host's calls; never shipped |
| L5 | `anyview` | exists | the binary: the runtime (the worker pool, the actors and delivery to the UI thread), the command line, single instance, the windows, the players and the desktop's now-playing entry (`media`), and the host that carries out what the windows ask through the platform |

### Allowed edges (workspace crates and quire; everything else is forbidden)

| Crate | May depend on |
| --- | --- |
| `anyview-core` | `ds-core` (its `#[derive(Word)]` is re-exported by `ds-core`, so `ds-core-derive` is not an edge) |
| `anyview-plugin-protocol` | nothing in the workspace: bayonet without its host half (the wire), `serde`, `thiserror` |
| `anyview-plugin` | `anyview-core`, `anyview-plugin-protocol`, bayonet (`host`) |
| `anyview-plugin-fake` | `anyview-plugin-protocol` (its tests also take `anyview-core`, `anyview-platform`, `anyview-plugin` as dev-dependencies) |
| `anyview-tool-kit` | `anyview-plugin-protocol` |
| `anyview-heif`, `anyview-raw` | `anyview-plugin-protocol`, `anyview-tool-kit` (their tests also take the host's crates as dev-dependencies) |
| `anyview-ffmpeg` | `anyview-plugin-protocol` (its tests also take `anyview-core`, `anyview-platform`, `anyview-plugin` as dev-dependencies) |
| `anyview-store` | `anyview-core` (and `rustix`, for the no-replace rename, extended attributes and `kill(pid, 0)`: safe wrappers, no `unsafe` here) |
| `anyview-fs` | `anyview-core` |
| `anyview-ui` | `anyview-archive` (an office document's facts and picture), `anyview-book`, `anyview-core`, `anyview-fs` (`OnDisk`), `anyview-image`, `anyview-pdf`, `anyview-peek` (without `pane` and `media`: `probe`, so the window and the launcher's pane tell a zip document from an archive by one rule, `peek` for the card of a file no stage shows, and `StillSource`, the host's small picture of a file), `anyview-store` (`file_details`, the General section of the Info tab), `anyview-text`, `ds` (the components and hooks), `ds-blitz` (the window, `TextureLayer`, and its `pdf` feature, which lays a book's chapters out on pages), `ds-core` (the `Machine` trait and `Stamp`), `ds-shell` (the missing-tool sheet, `HelperSheet`) |
| `anyview-media` | `anyview-core`, `ds-core` (`Word`, for the closed vocabularies); with `audio`, `symphonia` (the decoders `anyview-peek` already links for probing) and `cpal` (the sound card) |
| `anyview-image` | `anyview-core`, `ds-core` (`Word`, for the facts' labels) |
| `anyview-text` | `anyview-core`, `anyview-fs` (`OpenFile`, for a window of lines read by offset), `ds-core` (`Word` for token classes, and `base64` for `data:` URLs) |
| `anyview-platform` | `anyview-core`, `anyview-plugin`, `anyview-plugin-protocol`, bayonet (`host`), `ds-core` (`Word` for the closed vocabularies); with `quire-desktop`, `docket-client`, `docket-core`, `porter-core` and `prov` (docket's app side, section 2n) |
| `anyview-pdf` | `anyview-core` |
| `anyview-export` | `anyview-core`, `anyview-image`, `anyview-pdf`, `anyview-store` (`free_beside`, `link_new`, `partial_beside`, `sweep_leftovers`: the one way to claim a name), `anyview-text`, `ds-blitz` (`pdf`, behind the `print` feature: the printer of a text document), `ds-core` (`Word`, for the extension's slug) |
| `anyview-archive` | `anyview-core`, `ds-core` (`Word` for entry kinds) |
| `anyview-book` | `anyview-archive`, `anyview-core`, `ds-core` (`base64`, for `data:` URLs) |
| `anyview-font` | `anyview-core` |
| `anyview-peek` | `anyview-archive`, `anyview-book`, `anyview-core`, `anyview-font`, `anyview-fs` (`OnDisk`, `is_regular`, `open_regular`: the peek is the adapter that opens a path), `anyview-image`, `anyview-text`, `ds-core` (`Word`, `Size`, `Scale`); with `pane`, `ds` (the pane's components) and `ds-blitz` (`TextureLayer`, and the `pdf` feature's page cache) |
| `anyview` | `anyview-core`, `anyview-export` (the exports and printouts of images, PDFs and text), `anyview-image` (`Rgba8`, the picture a cached thumbnail lends the first frame, the encode of a saved frame, and `edited`, a picture's bytes after an edit), `anyview-media` (features `player` and `audio`), `anyview-pdf` (`apply`, a PDF's bytes after a page edit), `anyview-peek` (with `media` and `pane`; a recording's facts from its header when no plugin reads it, a window's size from a header, and `StillSource`, over the thumbnail cache), `anyview-fs` (`OnDisk`), `anyview-platform`, `anyview-plugin` and `anyview-plugin-protocol` (the registry and the plugins' export requests), `anyview-store`, `anyview-ui`, `ds` (`Appearance`, `WindowHost`), `ds-blitz` (`launch_idle`, `AppHandle`, `LastWindowClosed`, the clipboard), `ds-desktop` (`Desktop::probe`: whether PackageKit answers, for the Install... offer), `ds-helpers` (the catalog of tools, the probe and the PackageKit install of a missing one: the one crate that reaches PackageKit, on the bus `anyview-platform` otherwise owns; the binary names none of zbus) |

Dev-dependencies follow the same table, plus `wgpu` and `pollster` for `anyview`'s media-thread test (they never reach its normal build; they make the window's device and read a texture back), plus `tempfile` for `anyview-media`'s driver tests, plus `ds-harness`, `image` and `tempfile` and `anyview-platform`'s `testing` fakes for `anyview`'s window tests, plus `serde_json` for round-trip tests and `ds-core` with
its `testing` feature for `word_matches_serde` (`anyview-core`), and `tempfile` for scratch
directories (`anyview-store`, `anyview-fs`, `anyview-image`, `anyview-text`, `anyview-platform`, `anyview-peek`). `anyview-peek` also takes
`ds-harness` (a real Blitz document, and the hybrid GPU painter), `ds-lint` and `dioxus-ssr` as
dev-dependencies. `anyview-pdf` has none: its tests build their fixture in memory. `anyview-export` takes `tempfile`.

### Features

No crate turns a heavy dependency on by default: a default build of a library is its types and its
portable code, and the binary (or any other embedder) names what it links. `scripts/check-boundary.sh`
measures each crate with the features in this table (`flags_of`), so its checks see the tree a consumer gets.

| crate | feature | what it adds | the viewer |
|---|---|---|---|
| `anyview-platform` | `quire-desktop` | D-Bus single instance, the media session, portals, the file manager, the freedesktop entries (Linux; docket's app side) | on, through `anyview/quire-desktop` |
| `anyview-platform` | `testing` | the fakes and the private bus | tests only |
| `anyview-media` | `player` | the typestate player over the person's mpv | on |
| `anyview-media` | `audio` | the built-in audio player (symphonia, cpal) | on, through `anyview/audio` |
| `anyview-peek` | `media` | a recording's header and cover (symphonia, mp4parse, matroska-demuxer) | on |
| `anyview-peek` | `pane` | the pane that draws (`ds`, `dioxus`, `ds-blitz`) | on |
| `anyview-export` | `print` | a text document printed to a PDF (`ds-blitz`'s `pdf`) | on |
| `anyview-image` | `encode` | the AVIF and metadata encoders | on |
| `anyview` | `audio`, `quire-desktop` | forward the two above | both on by default; `--no-default-features` is the portable build |

### External boundaries (`scripts/check-boundary.sh`)

| Crate | Never reaches |
| --- | --- |
| `anyview-core` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, `ffmpeg-sys-next`, `image`, `syntect`, `blitz-dom`, `anyrender`; `serde_json` outside tests |
| `anyview-store` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, `image`, `blitz-dom`, `blitz-paint`, `anyrender`: blocking file I/O only, so the launcher links it cheaply |
| `anyview-image` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, the `blitz-*` crates, `anyrender`, `syntect`, `pulldown-cmark`: blocking decode and encode on the caller's worker, no spawning, no clock |
| `anyview-text` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, the `blitz-*` crates, `anyrender`, `image`, `resvg`, `jxl-oxide`: blocking reads on the caller's worker, no spawning, no clock |
| `anyview-peek` | `mpv-wgpu-player`, `rsmpv`, `rsmpv-sys`, `ffmpeg-next`, `ffmpeg-sys-next`, `zbus`, `ashpd`, `cpal`, `alsa`, `alsa-sys` anywhere in its tree: no libmpv, no libav, no D-Bus and no sound card in the launcher's process, and no `anyview-media` at all (it probes with `symphonia`; playing is the media crate's). Its `media` feature (off by default; the viewer and the launcher ask for it) pulls in `symphonia`, `mp4parse` and `matroska-demuxer`, pure-Rust readers of a recording's header (section 2f); without it the crate leaves them out and a recording is peeked as facts only. `wgpu`, pdfrum and `tokio` are in its tree (they come with `ds-blitz`, which the launcher links) but it never names them itself: the DIRECT table of the script. Its tree is held to a package-count budget, and so is its headless tree (`--no-default-features`: no `pane`, so no `ds-blitz`, `dioxus` of its own or renderer, and no `anyview-image` encoder), which reaches none of `ds-blitz`, `wgpu`, `rav1e` or `zbus` and builds on macOS and Windows |
| `anyview-pdf` | `dioxus`, `tokio`, `zbus`, `wgpu`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, the `blitz-*` crates, `anyrender`, `image`, `resvg`, `jxl-oxide`, `syntect`, `pulldown-cmark`, `rayon`: the one crate that names pdfrum. It draws to CPU pixels with the vello-cpu rasterizer and never encodes them (`anyview-image` owns every raster encoder), spawns nothing and has no pool |
| `anyview-export` | `zbus`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, `ffmpeg-sys-next`: blocking work on the caller's worker, no spawning, no clock. It names none of `pdfrum`, `image`, `wgpu`, `tokio` or the renderer itself (the DIRECT table of the script): the PDF comes through `anyview-pdf`, the pixels through `anyview-image`, and the printed page through `ds-blitz`'s `pdf`, which only the `print` feature links |
| `anyview-ui` | `zbus`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`: the player and the platform reach the views as `MediaHost`, `anyview-platform` traits and `HostRequest`s, never as dependencies. It never names `pdfrum` itself either, though `pdfrum` is in its tree through `anyview-pdf`. `tokio` and `wgpu` arrive only through `ds-blitz`, `image` through `anyview-image` and `pdfrum` through `anyview-pdf` (the DIRECT table of the script); the library never names them. The machine modules inside it (below) stay pure: the script fails on a source file of one that names Dioxus, quire's components, a decoder, the disk, a thread or a clock |
| `anyview-archive` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, `ffmpeg-sys-next`, the `blitz-*` crates, `anyrender`, `image`, `resvg`, `jxl-oxide`, `syntect`, `pulldown-cmark`, `skrifa`: blocking reads on the caller's worker inside a byte budget, no spawning, no clock; the one crate that names the container codecs |
| `anyview-book` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, `ffmpeg-sys-next`, the `blitz-*` crates, `anyrender`, `image`, `resvg`, `jxl-oxide`, `syntect`, `pulldown-cmark`, `skrifa`: blocking reads on the caller's worker, no spawning, no clock; it names no codec (the zip comes through `anyview-archive`) and decodes no picture |
| `anyview-font` | the same, and the archive codecs (`zip`, `tar`, `sevenz-rust2`, `flate2`, `bzip2`, `ruzstd`, `lzma-rust2`): the one crate that names `skrifa` for reading a face |
| `anyview-platform` | `dioxus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, `image`, the `blitz-*` crates, `anyrender`, `syntect`, `pulldown-cmark`, `resvg`, `jxl-oxide`: the edge knows the desktop, not the pictures; it spawns no thread and runs on the binary's tokio runtime |
| `anyview` | `rsmpv`, `rsmpv-sys`, `ffmpeg-next`, `ffmpeg-sys-next` anywhere in its tree: it links no libmpv and no libav, and runs the person's mpv and the FFmpeg plugin as programs. It never names, in its own manifest, `cpal`, `symphonia`, `zbus`, `ashpd`, `freedesktop-*`, `wgpu`, `mpv-wgpu-player`, `rsmpv`, `rsmpv-sys`, `ffmpeg-next`, `ffmpeg-sys-next`, `pdfrum`, `image`, the `blitz-*` crates, `anyrender` or `dioxus-native` (the DIRECT table): the bus, the renderer and the decoders come through the platform and the window crates. It does name `dioxus`, for the root component every window shares, and is exempt from the "only `anyview-platform` reaches `zbus`" check for the same reason it links that crate; the DIRECT row holds it to not naming it. The runtime inside it stays generic over the back ends and names none of them |
| `anyview-media` | `dioxus`, `tokio`, `zbus`, `pdfrum`, `image`, `syntect`, `pulldown-cmark`, `resvg`, `jxl-oxide`, the `blitz-*` crates, `anyrender`: the one crate that names `mpv-wgpu-player` (with its `player` feature), built with the `subprocess` host only, so no libmpv and no `rsmpv`. Its `audio` feature names `symphonia` and `cpal`, which the launcher's library must never reach (`anyview-peek` links symphonia for probing and never `cpal`). It spawns no thread of its own (cpal's callback thread and the `null` output's clock are the only ones), reads no clock beyond that output's, draws nothing and has no runtime: the binary runs its driver on the media thread and its exports on the pool |
| `anyview-plugin-protocol` | `anyview-core`, `ds-core`, `toml`, and everything `anyview-core` never reaches: bayonet's wire (no `host` feature, so no `toml`), serde and thiserror only, so a plugin author's tree stays theirs |
| `anyview-plugin` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, `image`, the `blitz-*` crates, `anyrender`, `syntect`: pure values, no effects |
| `anyview-plugin-fake` | what the protocol crate never reaches, and `anyview-core`: a plugin knows the protocol and nothing of the viewer |
| `anyview-heif`, `anyview-raw`, `anyview-tool-kit` | the same as `anyview-ffmpeg` below, but `image` is theirs to name (they read the PNG, TIFF or PPM a tool wrote): and `libheif-rs`, `libheif-sys`, `libraw-rs`, `libraw-sys`, `rsraw`, `rawloader` and `rawler` are forbidden in every crate's tree, with the libmpv and libav bindings |
| `anyview-ffmpeg` | `anyview-core`, `anyview-media`, `anyview-platform`, `anyview-plugin`, `anyview-ui`, `anyview-peek`, `ds-core`, `ds`, `ds-blitz`, `toml`, `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `ffmpeg-next`, `ffmpeg-sys-next`, `image`, `blitz-dom`, `anyrender`, `syntect`: a plugin knows the protocol, `serde`, `serde_json` and `thiserror`, and runs programs; it never links the libraries those programs are made of |
| every crate | `rsmpv`, `rsmpv-sys`, `ffmpeg-next`, `ffmpeg-sys-next`: no binding of libmpv or libav anywhere in the workspace (the script checks every crate in `crates/` and `plugins/` and `Cargo.lock`), and `dev/no-linked-codecs.sh` reads `ldd` of the built binary for `libmpv` and `libav*` |
| every crate but `anyview-platform` | `zbus`, `ashpd`, `freedesktop-*`, and the macOS and Windows bindings (the script checks the `zbus`, `ashpd` and `freedesktop` names for every crate in `crates/` and `plugins/`) |

`anyview-image` depends on `image` (png and jpeg from the pinned block, gif, webp, bmp, tiff, ico, tga,
qoi, exr and hdr added by its own manifest), `psd` (Photoshop, MIT OR Apache-2.0), `icns` (Apple icons, MIT),
`jxl-oxide`, `resvg` (without text), `kamadak-exif`, `thiserror` and `ds-core`; with the `encode` feature
(which the viewer and its exports turn on) also `img-parts` and `ravif`. `anyview-text` depends on `syntect` (the pure-Rust regex engine, no
oniguruma), `pulldown-cmark`, `csv`, `serde_json`, `serde`, `encoding_rs`, `thiserror` and `ds-core`.

`anyview-archive` depends on `zip` (deflate only: the central directory needs no codec, extracting an entry does), `tar`, `flate2` (its pure-Rust back end), `ruzstd`, `lzma-rust2`, `bzip2` (its pure-Rust `libbz2-rs-sys` back end), `sevenz-rust2` (decoders only, no encoder), `thiserror`, `anyview-core` and `ds-core`; no C library. `anyview-book` depends on `anyview-archive`, `roxmltree` (the package documents of an EPUB as a read-only tree), `thiserror`, `anyview-core` and `ds-core`. `anyview-font` depends on `skrifa`, `miniz_oxide` (the zlib streams of a WOFF), `thiserror` and `anyview-core`.

`anyview-peek` depends on the five crates below it, `anyview-fs`, `ds-core` and `thiserror`; its `pane` feature adds `ds`,
`ds-blitz` (feature `pdf`) and `dioxus`. Outside `pane/` (and the PDF raster that feeds it) no module names a type of
`ds` or dioxus: the PDF page is the peek's own `PageLook` and `PageTrouble`, and the pane converts them to quire's
`PdfPage` at its boundary (`pane/page.rs`). The headless tree has no dioxus, `ds-motion` or `ds-style`, and
`scripts/check-boundary.sh` fails if one returns. `wgpu` is not an exception to its rule so much as a fact of `ds-blitz`: `TextureLayer` and the PDF
thumbnail cache both live there, the launcher is a Blitz window and already links the renderer, and the
pane reaches the device only through `ds_blitz::use_gpu`, never by naming a `wgpu` type (FINDINGS, "The pane
and `wgpu`").

`anyview-pdf` depends on `pdfrum` (the pinned block's facade, with its `markdown` feature added by the
crate's own manifest), `pdfrum-edit` (for `reorder_pages` and page-range writes, which the facade's
`DocEdit` lacks) and `thiserror`.

`anyview-core` depends on `serde`, `thiserror`, `infer` and `ds-core` and nothing else. It does no
I/O and reads no clock: a function that needs bytes takes them (`FileHead`, `ZipEntries`), and one
that needs the time takes it.

## 2. Modules inside `anyview-core`

`lib.rs` declares private modules and re-exports each public item once at the crate root; there is
no other public path. A module names only modules above it in this list.

| Module | Holds |
| --- | --- |
| `error` | `CoreError`, the crate's one error |
| `units` | page, section (`SectionIndex`, `SectionCount`: the chapter or comic page of a book), media (time, length, volume, speed, chapter, time range, bitrate), ratio, zoom, turn, content-space and pixel newtypes; all integer |
| `media` | what a player says of a recording: `StreamKind`, `MediaTrack`, `TrackPlay`, `MediaChapter`, `VideoPresence`, `MediaTags` |
| `source` | `FilePath`, `FileName`, `FileStamp`, `Source` (a file's serialisable identity), `ReadAt` (bytes read by offset: a byte slice, a `Vec`, an `Arc`, an open `File`, or anything a host implements it for) and `ReadAtStream` (a `Read + Seek` over one), and `Input` (what a peek reads: name, stamp, shared bytes and, when they are a file of their own, the path; made from `&FilePath`, `&Source` or a name and a `Vec<u8>`) |
| `kind` | `FormatKind`, `Mime`, `FormatDetail`, `SyntaxName`, `kind_of_mime` and the format families |
| `sniff` | `sniff`, `sniff_zip`, `Sniffed` and the head and entries they read |
| `sequence` | `NonEmpty`, `Sequence`, `moved`, `neighbours` |
| `edit` | `Edit`, `EditKind` |
| `trail` | `Trail`, `TrailIn`, `TrailOut`, `TrailStacks`: undo and redo for one file as a pure machine over the versions its saves kept, generic over what names a version |
| `action` | `FileAction`, `Reach`, `reach`, `shortcut` |
| `export` | the per-format export enums, `AudioTarget`, `ExportChoice`, `ExportJob` (its `Transcode` is a cut, a track or a conversion of a recording) and its payloads |
| `resume` | `Resume`, `TrackChoice` |
| `facts` | `Fact`, `Facts`, `FactLabel` (and where each label is listed by default: `group`, `tier`), `FactGroup` (the sections of the Info panel, in display order, `General` last), `Tier` (headline rows make the summary line, `Facts::summary`), `FactValue` and its constructors (size with exact bytes, date, coordinate, altitude, version), `FactTime`, `LocalZone` (the person's time zone, read from the system by `jiff`; every shown time is converted to it and never names a zone), `Coordinate`, `kind_name` (Finder's "JPEG image"), and `FileDetails` with `Facts::general` (the General section: kind, size, created, modified, where, where from, permissions); `label`, `group`, `value`, `when`, `place`, `summary`, `kind_name` and `file` are private |
| `peek` | `Peek`, `PeekBudget`, `Deadline` (the budget's time as an instant a long loop asks about; cooperative), `StageSupport` |
| `work` | `Backend`, `Stop`, `StopState`, `Ticket`, `Ticketed`: the contract with the threads. The one public module: reached as `anyview_core::work::X` |
| `APP_NAME` | the name a person sees ("Viewer"): the welcome title, the helper sheet, the now-playing identity. Identifiers (binary, app id, bus names) keep `anyview` |
| `profile` | the one match on `FormatKind`: `actions_for`, `edits_for`, `mime_for`, `stage_support` |

## 2a. Modules inside `anyview-store`

Same rules as section 2: private modules, each public item re-exported once at the crate root.
`io` is the only module that touches the disk; the others are pure, except the file-system modules
named below.

| Module | Holds |
| --- | --- |
| `error` | `StoreError`, `StoreOp` |
| `viewed` | `Viewed`, seconds since the epoch, handed in by the caller |
| `label` | `ResumeLabel` and `resume_label`, the row subtitle derived from a `Resume` |
| `history` | `HistoryCap`, `HistoryEntry`, `History` and the pure `history_after_view` |
| `record` | the per-file record, its hashed file name, `applicable` and `prune_decision` (private) |
| `io` | the effects: `Job` and `Done` (probe a file, make its first frame, open it, read a window of lines, search it, open a neighbour ahead, read a stamp, list a folder), `Workers` (the pool the binary owns), `Work` (with its `WorkLane` and `WorkKind`), `Reply`, `Edge` (what one window is wired to), `HostRequest` (what it asks of the binary, among them `Provide`: install this tool), `Need` (a `Needs` row with the tool that would answer it), `HelperSource` and `HelperWords` (what the host lends the sheet to word it), `ResumeSource` and `FileLocks` (what the binary lends it to read; the small picture it lends for a first frame is `anyview_peek::StillSource`), `folder_sequence`, `Backend` and `Stop` |
| `reader` | `read_history`, `HistoryRead`: the API the launcher links |
| `writer` | `StoreWriter`: `record_view`, `save_resume`, `load_resume` |
| `save` | the save pipeline: `Pending`, `BackedUp` (consumed by `write_in_place`), `Written`, `Durability` |
| `versions` | `Versions` (back up, list, restore), `Version`, `VersionId`, `KeepPeriod`, `SavedAt`; kept versions are private (`0700` folders, `0600` files), a version identical to the file's latest is not made again, a sidecar that cannot be read is skipped by `list` |
| `prune`, `rekey` | `Versions::prune` (by age, the half-made files of a crash, then the size cap `DEFAULT_CAP`, a file's newest version never; nothing by age when the clock is over a year past the newest version) and `Versions::rekey` (a renamed file's versions follow it) |
| `place` | `free_beside`, `is_free`, `rename_noreplace` (`renameat2` with `RENAME_NOREPLACE`, a link and unlink where the file system has none), `link_new`, `copy_new`, `partial_beside`, `is_taken`: Rename, Duplicate and every export claim names here, never by looking first |
| `details` | `file_details`: a file's size, dates, folder, permissions and the address it was downloaded from (the `user.xdg.origin.url` attribute), as `FileDetails`; best effort, a field the file system lacks is absent |
| `attrs`, `sweep`, `guard`, `original` | the owner, extended attributes and ACLs a save and a copy keep, and `is_read_only`; `sweep_leftovers` (temporary files of dead processes); one save of a real path at a time in the process; a file's identity at backup time, for the stamp check before the rename |

A save writes a temporary file beside the original (its mode, owner, xattrs and ACLs, named for the
process and a counter), syncs it and renames it over; a file with other hard links is written in place
after the backup so the links stay one file; a read-only file is refused. If the file changed since the
backup, what it holds then is kept before it is replaced. A save that fails before it touched the file
drops the copy it kept. A folder sync that fails after the rename is `Durability::Unconfirmed`, not an
error.

On disk, under a root the caller names: `history.json` (the `History`, newest first, capped) and
`resume/<hash of path>.json` (one record per file: path, `FileStamp`, `Resume`). A file is written
whole to `<name>.tmp` beside it, synced, and renamed over the old one, so a reader never locks and
never sees a partial file. There is no database: `redb` takes an exclusive file lock that blocks the
launcher while the viewer runs. The viewer is the one writer.

## 2b. Modules inside `anyview-ui`

Same rules as section 2: private modules, each public item re-exported once at the crate root. Each
region is a directory with `model.rs` (the states, inputs, outputs and params), `step.rs` (the
`Machine` impl) and `tests.rs`; a table in a test file is exempt from the size aim.

| Module | Holds |
| --- | --- |
| `chrome` | `Chrome`, `PinReasons` (a never-empty set), `ChromeParams` |
| `panel` | `Panel`, `PanelTab`, `PanelTabs` |
| `palette` | `Palette`, `PaletteParams` (the ranked rows) |
| `context` | `ContextMenu` (closed, or open with its corner), `ContextParams` (the rows and where a key opens it), `ContextPick`, `ContextEntry`, `Spot`, and `entries`: which of the palette's commands a Mac's context menu shows, in what order, under what titles, with its rules |
| `sheet` | `Sheet`, `ExportDraft` (one format's export choice), and the install question for a missing tool: `Sheet::Helper` with `HelperPhase` (`Ask`, `Installing`, `Failed`, `NotFound`, `Unsupported`), `SheetIn::OfferHelper` and `HelperEnded(Helper, HelperEnd)`, `SheetOut::Provide` and `Reopen` |
| `navigate` | `Navigate` over the core `Sequence`; `Leave` ends a walk when a dropped file is not one of the list |
| `presentation` | `Presentation` |
| `load` | `Load`, `Ticket`, `freshness` (whether a file on disk is still the one opened: the decision behind a reload) |
| `stage` | `Stage` and its six machines (`raster`, `pdf`, `media`, `text`, `table` (the sheet and the row the cursor is on) and `tree` (which nodes are open); the last two ask nothing of the window, so their `Out` types are empty; the media one steps on the player's events as `PlayerEvent`s and answers in `PlayerCommand`s, both the machine's own types), the shared `find` and `zoom` parts, `dispatch` (a command or a key becomes an input for the stage that is showing) and `resume` (the place a stage keeps, and the input that puts one back) |
| `keys` | `route`, `Route`, `Regions` |
| `viewer` | `Viewer`, `ViewerIn`, `ViewerOut`: the root |
| `command` | `Command` (a file action or a stage command), `StageCommand` and its keys |
| `typed` | `TypedText`: a query or a name, a static literal or typed |
| `io` | the effects: `Job` and `Done` (probe a file, open it, read a window of lines, draw tiles of a PDF), `WorkLane` (how soon a job is wanted: `Job::lane` is its one decision), `Workers` (the pool the binary owns), `Work`, `Reply`, `Edge` (what one window is wired to), `HostRequest` (what it asks of the binary), `Backend` and `Stop`; `media` is the seam to the player: `MediaHost` (starts one for a file), `MediaLine` (what a window holds of it), `MediaNotice` (what it reports, in the machine's terms), `MediaWake`, `SlotPixels` |
| `families` | the full tier: `StageView` (one implementation per family of formats), the registry (`visit`, `family_of`, the one match on `FormatKind`), the views `raster`, `text`, `table` (a header over a `VirtualList` of rows, a sheet list in the panel's Contents tab), `tree` (visible nodes in a `VirtualList`, JSON Lines as one tree of its lines), `pdf` (and, in `pdf/bound`, a book bound as a PDF), `media` and `peek_only` (office facts and the document's thumbnail), `card` (`InfoCard`, the one file card: the Info panel's Info tab and the facts-only stage draw it) and `found` (the capsule's part of a find: the `3 of 17` readout and the steps; finding itself is the palette's, below). A family's `hit_lines` gives the palette the places the find found, as `HitLine`s. The media view: `MediaDoc` (the player started for a file), `MediaShelf` and `MediaLive` (what the window last heard of it: position, volume, tracks, chapters, trim marks), the capsule's slots, the panel's Tracks and Chapters tabs, and the album card of an audio file with no picture |
| `views` | the window: `ViewerApp`, `Launch`, `WelcomeApp` (the window of a launch with no file: an Open button, a drop target, ⌘O); `failed` (the screen of a file that did not open: the reason in words, and Show in Folder where there is a file manager); `window` (the component), `shelf` (the results the window holds, and `Dispatch`), `carry` (what each output of the root does), `arrive` (each result of a worker as an input), `effects` (what waits on a probe or the device), `preloads` (the files opened ahead); the chrome, the palette, `context` (the right-click menu: quire's `Menu` with `MenuPlacement::Context`, placed at the point the machine holds), the panel (the body of the window's left `SplitView` pane, never quire's `SidePanel`: it shrinks the stage, not the window, and its width is the split view's), the sheets, key events as shortcuts, `stylesheet` (one sheet for each concept: `window.css`, `peek.css`, `sheets.css`, `welcome.css`, and one beside each family's view) |

## 2c. Modules inside `anyview-image`

Same rules as section 2: private modules, each public item re-exported once at the crate root.
Everything is blocking and runs on the caller's worker: no runtime, no spawning, no clock. `decode`
and `decode_bytes` are the one way pixels come out, and `encode` the one way they go in.

| Module | Holds |
| --- | --- |
| `error` | `ImageError` |
| `pixels` | `Rgba8` (straight alpha), `PremultipliedRgba8`, and the one conversion between them |
| `orientation` | `ExifOrientation` (a `Mirror` then a clockwise `QuarterTurn`), its tag table and `applied` |
| `exif` | `ExifFacts`, `Exposure`, `Ratio`, `SignedRatio`, `Flash`, `Location`: read with `kamadak-exif` (camera, lens, settings, bias, flash, software, copyright, density, and the GPS block; the body and lens serial numbers are never read); `ExifFacts::camera_facts` are the rows a preview may list; `ExifFacts` holds no place, and `Location::of_file` (read by `picture_facts` alone) is the place, which only the viewer's own Info panel lists; `format` words them; `patch` writes the orientation entry (private) |
| `picture_facts`, `resolution` | `picture_facts`: the colour, density, camera and location rows of the viewer's Info panel; `Resolution`: dots an inch from EXIF, a PNG `pHYs` chunk or a JPEG's JFIF block |
| `scale` | `resized` (the export's `Resize`); peek-budget fitting (private) |
| `decode` | `decode`, `decode_bytes`, `declared_size` (the upright size from the header and EXIF alone), `Decoded` (a still, an `Animation` with its `Plays`, or a `HeldStill` when the frames pass the 256 MiB cap), `Frame`, `FrameCount`, `ColourInfo`; `codec` is the one match on `RasterFormat`; `ceiling` (the peak memory a decode may hold, checked from the header before any pixel is decoded), `stills`, `plays` (loop counts from the container), `frame_count` (frame counts from the container, without decoding), `highrange` (EXR and HDR, tone mapped with extended Reinhard then sRGB), `layered` (Photoshop composite, largest icon of an ICNS), `jxl`, `svg`, `svg_limits` (a drawing's filter work and nested pictures, checked from its tree before it is drawn) and `look` are private |
| `decode::natural` | `natural_size`: the upright size a picture shows at, read from the first 256 KiB of the file (a header and its EXIF orientation, or an SVG's root tag); nothing is decoded and the claim is not checked against a decode's ceiling |
| `peek` | `RasterPeek` and `VectorPeek` (the two `Peek` implementations), `ImagePeek`, `PeekedFormat` |
| `encode` | `encode`, `encode_bmp`, `encode_with_metadata`; `codecs`, `avif` and `metadata` (EXIF and ICC splicing with `img-parts`) are private |
| `export` | `plan_export` (an image choice as `ExportJob`s: pure), `encode_file` (a file resized and encoded, keeping its metadata or not), `ImageFile` (a file's, or a comic entry's, bytes and what it sniffed as: its upright picture, an SVG's declared size, whether a JPEG is already upright) |
| `rotate` | `rotate_jpeg`, `flip_jpeg`: lossless rotation and mirroring by rewriting the EXIF orientation segment |
| `edit` | `edited`: an `Edit` applied to a picture file: a JPEG through its orientation tag alone, PNG, WebP, TIFF and BMP decoded, moved and written again with their metadata; animations and the formats with no lossless writer are refused |

**Alpha** is straight (not premultiplied) everywhere in this crate; `Rgba8::premultiplied` is the
conversion a GPU compositor needs, and `PremultipliedRgba8` is a distinct type so the two cannot be
mixed. **Orientation** is applied in the decoder: pixels come back upright and nothing downstream
reads the EXIF tag. Encoding takes upright pixels, so a carried EXIF block has its orientation
reset to upright. A JPEG rotated "in place" is not decoded at all: `rotate_jpeg` rewrites one APP1
segment and copies every other byte.

The `avif` feature adds AVIF decoding through the `image` crate and the dav1d C library (BSD-2,
linked dynamically, found with pkg-config). It is off by default, so the default build needs no C
toolchain pieces; without it an AVIF file is `ImageError::NotCompiledIn`. AVIF encoding is `ravif`
(pure Rust, and with `rav1e` the largest tree in the crate), built only with the `encode` feature: that
feature gates every way pictures go out (the `encode` functions, `edited` and the exports) and the
metadata carried across a re-encode. A caller that only looks at pictures, as `anyview-peek` does,
links none of it.

## 2d. Modules inside `anyview-text`

Same rules as section 2. Blocking and effect-free except `bytes::FileBytes`, which reads ranges of
one file.

| Module | Holds |
| --- | --- |
| `error` | `TextError` |
| `bytes` | `ByteSource` (read a range), `FileBytes`, `HeldBytes` |
| `encoding` | `TextCodec`, `detect` (byte-order mark, UTF-8 validity, Windows-1252 fallback), `Coverage`, `Detected` |
| `lines` | `TextLines`, `LineCount`: a sparse line index (every 64th line) so any window of lines is read and decoded without the rest |
| `find` | `Needle` (a phrase, lower-cased, never empty), `FindHit` (a line and the bytes of it a phrase covers), `ByteOffset`, `MAX_HITS`; `TextLines::find` reads the file once, in batches |
| `code` | `Highlighter`, `SyntaxId`, `CodeLines` (windowed highlighting with saved parser states), `TokenClass`, `TokenSpan`, `TokenLine`, `tokens_html`; `class` is the one table from syntect scopes to classes, `state` and `html` are private |
| `markdown` | `render`, `Rendered`, `RenderEnv`, `LocalFiles`, `NoFiles`, `DiskFiles`, `Heading`, `HeadingLevel`, `Anchor`; `events` (the safety pass), `images`, `links` and `outline` are private |
| `table` | `Table`, `HeaderMode`, `RowCount`, `RowIndex`, `ColumnCount`, `Separator` (comma, tab, semicolon or pipe, chosen by the rows), `CharWidth` and the read caps (`TABLE_BYTES`, `TABLE_ROWS`); `Workbook` and `Sheet` (calamine: XLSX, ODS, XLS, one `Table` per sheet, capped by `SHEET_ROWS` and `WORKBOOK_CELLS`; XLSX and XLSB are streamed cell by cell, so a sheet that claims a grid it does not hold costs the cells it has; `open_start` reads the first sheet's start for a peek); `header` (the guess) is private |
| `tree` | `Tree` (`read`, `children`, and `visible` / `visible_count`, the document flattened under an `OpenNodes` set and cut to a window), `TreeRow`, `VisibleRow`, `Openness`, `RowLabel`, `NodeKind`, `ChildCount`; `TreePath` and `OpenNodes` live in `anyview-core`; `node` and `rows` are private |
| `peek` | `PlainPeek`, `CodePeek`, `MarkdownPeek`, `TablePeek`, `TreePeek` and their `*Peeked` types, `Tally`, `PEEK_LINES` |
| `export` | `plan_export` and `plan_print` (a text choice, or a printout, as `ExportJob`s: pure) and `printable_html` (a Markdown, code or plain-text file as the whole page `ds_blitz::pdf` prints: Markdown rendered with its local images inlined, code highlighted, text as it is; `print.css` is its stylesheet, with fixed colours since paper is white) |
| `escape` | HTML escaping, the one place it is written (private) |

Token classes are words (`keyword`, `string`, `comment`, …), never colours: the stylesheet maps each
`tok-<class>` to a design-system colour token. Markdown never passes raw HTML through (it is shown
as text), keeps only web, mail and relative link targets, and writes local images as `data:` URLs
read through `LocalFiles`, because quire's sealed frames load nothing else.

## 2e. Modules inside `anyview-platform`

Same rules as section 2: private modules, each public item re-exported once at the crate root; the
implementations are reached through `portable`, `linux` and `testing`. A second platform adds `macos/` or
`windows/` beside `linux/` and selects it in `lib.rs`; no other crate changes.

**Two halves, one feature.** The owner's rule (quire design/36) is that everything but the desktop itself is
cross-platform and the desktop's services are additive extras. So `portable` is built everywhere and calls no
desktop service, and `desktop` (public path `anyview_platform::linux`, kept for `sill`) is the Linux desktop's services (D-Bus, the portals, MPRIS, the freedesktop
application entries), built only with the feature `quire-desktop` on Linux. `quire-desktop` is opt-in
(the default build is the traits, the portable stand-ins and the `testing` fakes; Cargo has no per-target default
features, so the code behind it is also gated on `target_os = "linux"`); the crate's `zbus`, `freedesktop-desktop-entry`, `memfd` and `futures-util` are optional
dependencies of that feature. The binary forwards it (`anyview/quire-desktop`, on in its defaults), and `--no-default-features` is the
portable build. `anyview-platform` is declared in the workspace with `default-features = false`, so only the
binary's feature decides it; `sill` takes the crate by path and asks for `quire-desktop`. Without the feature each
desktop ability is a trait the host still holds, answered by a portable stand-in:

| Ability | With `quire-desktop` (Linux) | Without it |
| --- | --- | --- |
| Single instance | `DbusInstance`: the name `org.quire.Anyview1`, with bus activation (the `.service` file starts the viewer for a call) | `LatchkeyInstance`: the per-user socket |
| Link | `SystemOpen` (`xdg-open`; `linux::XdgOpen` is the same type) | `SystemOpen` (`xdg-open`, `open`, `explorer`) |
| Show in Folder | `FileManagerReveal` (`FileManager1.ShowItems`, the file selected) | `SystemReveal` (`open -R`, `explorer /select,`, else the folder through `xdg-open`) |
| Share | `MailShare` (`xdg-email`) | `NoShare`: no targets |
| Print | `PortalPrinter` | `NoPrinter`: `PrintOutcome::NoDialog` |
| Open... | `PortalPicker` | `NoPicker`: `PickOutcome::NoDialog` (no portable dialog yet; a later lane) |
| Now playing | `MprisSession` | none: `NowPlaying::Absent` |
| Thumbnails, window stacking | `FreedesktopThumbnails` and `NoStacking`, re-exported from `linux` | the same, from `portable` |

**What a file offers, declared once.** The commands of a file are the intersection of four tables, and the
palette, the context menu, the capsule and the keys all read the result (`views/session.rs` `commands`, then
`context::entries`, `offered_slots` and `ViewerParams::files`): the kind's actions (`anyview_core::actions_for`,
which the launcher shares), what the showing stage can do to its file (`Stage::abilities` in
`stage/abilities.rs`: the edits it carries out and the export it writes), what the file allows (a save in place, an
edit that can be made, a player) and what the platform has (`PlatformAbilities`). `session::offered` is the one
match over `FileAction`, so an action added to the vocabulary has to say where it applies. The stage's own
commands are the ones its dispatch answers (`Stage::input_for`). The context menu only orders and titles; the
capsule's file buttons are dropped when the file action is not listed. The table tests in `views/session.rs` hold
kind by command for the palette and the menu.

**What the platform can do, and who hides what it cannot.** `Picker`, `Printer` and `Reveal`
have `present()` (true unless the implementation only answers "not available": `NoPicker`, `NoPrinter`), and `Share` has its `targets()`. `host::Hosting::abilities()` reads them into
`anyview_ui::PlatformAbilities { pick_files, print, share, reveal }` (a plain set; `ALL` is the
default), which the window's `Wiring` gives its `Edge` (`with_platform`). The views read it, never the operating
system: `views/session.rs` `offered` filters the palette's commands (so the context menu and the file-action keys,
which derive from them, follow), `keys::Regions::pick_files` unbinds ⌘O, `StageCx::platform` gates the Show in Folder
button of the card and picture screens, `Offer::of` those of the failure screen, and the
welcome window drops Open.... The Install... of a missing tool is the host's: `HelperHost::installing_where` is
quire's `Helpers` capability from `ds-desktop`, probed once in `program::start` (`ds-desktop/dbus` is on with
`quire-desktop`; without it every capability is absent). `ds-desktop` describes quire's own services, so it is not
consulted for the portals above.

Single instance without the bus (`portable::LatchkeyInstance`, on latchkey): the first launch takes the
advisory lock and listens on `$XDG_RUNTIME_DIR/anyview/agent.sock` (macOS: under `$TMPDIR`; Windows: a named
pipe); a later launch finds the lock held, connects, writes one line of JSON (`{"op":"open","files":[..]}`,
`peek`, `play`, `handoff` with the same fields as the bus method) and reads one line back (`{"ok":true}` or
`{"ok":false,"why":..}`), then leaves. The viewer parses everything it receives and refuses what the bus
service refuses (a relative path, a place that is not a `Resume`); a line is cut off at 16 MiB. A killed viewer
leaves nothing to clean: the kernel drops the lock, and the next launch removes the stale socket under it.
Nothing starts the viewer when none runs, which the bus does; the launch that finds nobody home is the
viewer. Linux with `quire-desktop` keeps the bus because activation (a launcher's call starts the viewer) and
`forward_over` (the launcher's own connection) are the bus's, and latchkey starts nothing by itself. Nothing reads
`std::env`, `dirs` or a bus address outside `env`: the binary builds an `Env` (`Env::from_process`, then `with_dirs`, `with_session`, `with_spawn`, `with_audio_output`, `with_tool_path`, `with_window_screen`; `PATH`, `ANYVIEW_AUDIO_OUTPUT` and `ANYVIEW_WINDOW_SCREEN` are read there and nowhere else; `Env` is `#[non_exhaustive]`),
a test builds its own (`Env::isolated`, which names no bus and refuses to start programs). Async
methods are driven by the caller's runtime and never spawn; blocking ones (thumbnails)
run on the caller's worker.

| Module | Holds |
| --- | --- |
| `error` | `PlatformError` (`NoBus`, `Bus`, `Io`, `Thumbnail`, `Spawn`, `Exec`, and the `Plugin*` family: `PluginSilent`, `PluginCrashed`, `PluginProtocol`, `PluginVersion`, `PluginLacks`, `PluginFailed`, `PluginCancelled`), `IoOp` |
| `env` | `Env` (`dirs`, `session`, `spawn`, `audio_output`: `ANYVIEW_AUDIO_OUTPUT`, as written), `Dirs`, `BusRoute` (`Usual`, `Address`, `Absent`) |
| `spawn` | `Argv`, the `Spawn` trait, `ProcessSpawn`, `RefuseSpawn` |
| `uri` | `file_uri`: the escaped `file://` URI the thumbnail spec hashes and the file manager takes |
| `instance` | `Instance`, `Request` (`Open`, `Peek`, `Play`, `Handoff`), `Handoff`, `Claim`, `Primary` (std channel: `next` blocks, `next_within(Duration)` gives up; an async program reads it on a thread of its own, as `program::relay` does) |
| `handoff` (private) | a `Handoff` as the fields both wires carry (`Wire`): encoded and parsed once, for the bus and the socket |
| `media` | `MediaSession`, `MediaState`, `MediaControl`, `PlaybackStatus`, `Ability`, `SeekDirection`, `TrackSerial` |
| `thumbnail` | `ThumbnailCache`, `ThumbSize`, `ThumbPixels` |
| `printer` | `Printer`, `PrintOutcome` (`Printed`, `Cancelled`, `NoDialog`), `JobTitle` |
| `share` | `Share`, `ShareTarget` |
| `reveal` | `Reveal` |
| `picker` | `Picker`, `PickOutcome` (`Chosen`, `Cancelled`, `NoDialog`): the desktop's file dialog |
| `link` | `OpenLink`: a web or mail address handed to the desktop's handler |
| `plugin` | `discover` and `discover_in` (the same with a folder in place of the person's data directory) (`Discovery`, `Rejected`), `PluginRunner` (`probe`, `thumbnail`, `decode`, `export`, and the routing seam `peek_facts`), `Timeouts` (bayonet's), `PluginFacts`; the `Wire` that tells bayonet the viewer's messages is private |
| `stacking` | `WindowStacking`, `Stacking`, `StackingOutcome` |
| `portable` | built everywhere, no desktop service: `LatchkeyInstance` (`new`, `under(dir)` for tests; the `frame` module is its line of JSON), `SystemOpen`, `SystemReveal`, `FreedesktopThumbnails`, `NoStacking`, and the absent abilities `NoApps`, `NoShare`, `NoPrinter`, `NoPicker` |
| `desktop` (feature `quire-desktop`, Linux; quire design/36's module name), public as `linux` | one implementation per trait: `DbusInstance` (and `forward_over`, the call a launcher makes on its own bus connection; it also serves docket's `IntentProvider1`, `intents`, section 2n), `MprisSession`, `DesktopApps`, `PortalPrinter`, `PortalPicker` (the FileChooser portal), `MailShare`, `FileManagerReveal`; `FreedesktopThumbnails`, `NoStacking` and `XdgOpen` (an alias of `SystemOpen`) re-exported from `portable` under their old names; `portal` (private) is what every portal call shares: the request path, the `Response` code and the answer stream |
| `testing` (feature `testing`) | `FakeInstance`, `FakeMediaSession` (and `FakeMediaHandle`, its clonable test end, for when the session is given away), `FakeApps`, `FakeThumbnails`, `FakePrinter`, `FakePicker`, `FakeLinks`, `FakeShare`, `FakeReveal`, `FakeStacking`, `RecordingSpawn`; clones share their record. `PrivateBus` (a `dbus-daemon` with a configuration of its own) and `MprisClient` (the control center's end of the player) are the bus tests' rigs, and exist only with `quire-desktop` |

The trait shapes (a trait whose method awaits returns `impl Future + Send`, so a consumer is
generic over it rather than holding a `dyn`):

| Trait | Methods |
| --- | --- |
| `Instance` | `claim(&Request) -> Result<Claim>`: own `org.quire.Anyview1` (`Claim::Primary`, whose `Primary::next` yields what later launches forwarded) or forward the request to the owner (`Claim::Forwarded`) |
| `MediaSession` | `publish(&MediaState) -> Result<()>`; `next_control() -> Option<MediaControl>` |
| `ThumbnailCache` | `lookup(&FilePath, &FileStamp, ThumbSize) -> Result<Option<ThumbPixels>>`; `store(.., &ThumbPixels) -> Result<()>` |
| `Printer` | `print(&[u8], &JobTitle) -> Result<PrintOutcome>` |
| `Share` | `targets() -> Vec<ShareTarget>`; `share(&FilePath, ShareTarget) -> Result<()>` |
| `Reveal` | `reveal(&FilePath) -> Result<()>` |
| `WindowStacking` | `request(Stacking) -> StackingOutcome` |

On the bus: `org.quire.Anyview1` at `/org/quire/Anyview1` has `Open(as)`, `Peek(s)`, `Play(s)` and
`Handoff(s file, s resume, t results, as entries)` over absolute paths (a relative one is an
`InvalidArgs` error). A handoff is a file the launcher's pane was showing: `resume` is the JSON a
`Resume` is stored as (`Resume::Nothing` when the pane held no place of its own, which leaves the
viewer to continue where the person last left the file), `entries` the paths of the search's
results in the order shown with `file` among them, and `results` the id of that search
(`SequenceOrigin::Results`), so ← and → walk the results; no entries means no list. A file that is
not among its entries, or a place that is not a `Resume`, is an `InvalidArgs` error. The launcher
calls it with `forward_over(&connection, &Request::Handoff(..))` on its own connection, so the
wire is written once, here. `dist/org.quire.Anyview1.service` is the activation file that starts
`anyview` when a call arrives while none runs. The player is
`org.mpris.MediaPlayer2.anyview` at `/org/mpris/MediaPlayer2`; its track id is
`/org/quire/Anyview1/Track/<TrackSerial>`. Thumbnails live at
`<cache>/thumbnails/{normal,large,x-large}/<md5 of the file URI>.png` with `Thumb::URI`,
`Thumb::MTime` and `Thumb::Size` text chunks. The tests of the bus implementations run against a
`dbus-daemon` the test starts with its own configuration (no service directories) and skip with a
message when the program is not installed.

`anyview-platform` depends on `latchkey`, `tokio` (channels, `spawn_blocking` and a timeout), `md-5`, `png`,
`percent-encoding`, `serde_json`, `dirs`, and, behind `quire-desktop` on Linux, `zbus` (its `tokio` feature, so
the binary's runtime drives it), `freedesktop-desktop-entry`, `memfd` and `futures-util`; also bayonet with its `host` feature (finding the manifests and running the plugins: `poll` on the plugin's pipe, so it is read with a deadline and no thread, and the process group it is killed with), `thiserror`, `anyview-core`, `anyview-plugin`, `anyview-plugin-protocol` and `ds-core`.
## 2f. Modules inside `anyview-peek`

Same rules as section 2: private modules, each public item re-exported once at the crate root. The peeks
are blocking and run on the caller's worker (`worker` is that worker, for a host that has none); only
`pane` draws. Its `pane` feature (off by default; the viewer asks for it) is everything that draws or rasterises, `ds`, `dioxus` and
`ds-blitz`; without it the crate links none of them, and a PDF is peeked as facts only.

Every peek reads an `anyview_core::Input`: a path (made by `anyview_fs::OnDisk::on_disk` on a
`FilePath` or a `Source`, since `anyview-core` does no I/O and opens nothing) or any bytes a host injects
(`Input::from((name, bytes))`, or its own `ReadAt`). A back end reads it through `ReadAt` or
`Input::reader`; only two things need a real path, and each refuses cleanly when the input has none:
the folder peek (a directory has no bytes) and a thumbnail read from the desktop's cache (the host's
`StillSource`). A PDF takes the page cache when it has a path and `pdf_thumb_bytes` when it has not.

The front door is `look(src, &Peeking) -> AnyPeeked`: it probes the file and peeks at it. `Peeking` is
`#[non_exhaustive]` with `Default` (the launcher pane's settings, `Peeking::pane()`) and `with_*` builders for
the budget, the box a PDF page is fitted into (`fit`, a `PixelSize`) and the host's `StillSource`. `probe` and
`peek` stay for a caller that holds a `Probed` between the two (the viewer's window probes on one job and
peeks on another).

| Module | Holds |
| --- | --- |
| `error` | `PeekError` |
| `registry` | `KindVisitor`, `visit`: the one exhaustive match over `FormatKind` in the light tier |
| `body` | `Body` (the type-erased result), `Light` (a `Peek` whose result and error convert into `Body` and `PeekError`) |
| `probe` | `Probed`, `probe`: what a file is, from its first 4 KiB and, for a zip, its entries (`anyview_archive::zip_entries`); a zip that cannot be listed is a plain archive. It takes a path or any `Input`, as `peek` does |
| `any` | `AnyPeeked` (with `still()`, the picture a card shows) and `peek`: runs the registry's visitor, adds the size and the date, and turns a failure into `Body::Unavailable`; a video that shows only its facts takes the host's still |
| `looking` | `look`, `Peeking`: probe, then peek, as the settings say; one front door for the launcher's pane, the viewer's cards and a host's worker |
| `unavailable` | `Unavailable` (`Missing`, `Unreadable`, `TooBig { len, allowed }`, `Damaged(reason)`, with `label()` for the words): why a card has no peek |
| `frames` | `StillSource`, `NoStills`: the seam through which the host lends a small picture of a file the peek does not decode, a video's cached thumbnail in the launcher and a picture's first frame in the viewer (the peek names no platform crate, so the host reads the cache) |
| `media` | `VideoPeek`, `AudioPeek`, `MediaLook`; with the `media` feature, private `recording` (`Recording`, the header as plain values, and its rows), `audio` (symphonia: MP3, AAC and ALAC in M4A, FLAC, Ogg Vorbis and Opus, WAV, AIFF; tags, track number and the front cover), `mp4` (mp4parse over the `ftyp` and `moov` boxes alone, found by seeking over the rest: MP4, M4V, MOV), `matroska` (matroska-demuxer: MKV, WebM) and `peek` (which parser a container goes to, the cover reduced to the budget); AVI, WMV, FLV, MPEG-TS, MPEG and Ogg video go to no parser; without the feature, `absent` (`FactsPeek`) |
| `described` | `FactsPeek<K>` and one marker per kind with no back end yet (`OtherPeek`; `VideoPeek` and `AudioPeek` only without `media`): what sniffing established, nothing pretended |
| `office` | `OfficePeek`, `OfficeLooked`: the package's facts and its own picture (a document's thumbnail) as `Body::Picture` |
| `book` | `BookPeek`, `BookLook`: an EPUB's title, author, publisher, language and chapters, or a comic's page count, over the cover (the package's cover image, else its first image; a comic's first page), reduced to the budget; a book that cannot be opened still shows its type |
| `folder` | `FolderPeek`, `FolderSummary`: one level, item count, size and kinds |
| `natural` | `natural_size`: the size a picture or a video shows at, from the first bytes of the file (a picture's header through `anyview-image`, a movie's `moov` or Matroska header up to 8 MiB); any other kind, a path that is not a regular file and a header that does not say give `None` |
| `pdf` | `PdfPeek`, `PdfPeeked`, `PageLook`, `PageTrouble` (a page drawn, blank or failed, in no component's types): the first page, through `ds-blitz`'s thumbnail cache for a file and `pdf_thumb_bytes` for bytes handed in; facts only without the `pane` feature |
| `worker` | `PeekWorker` (`spawn`, or `looking` for `look` itself), `WorkerConfig` (`#[non_exhaustive]`, with `with_*` builders): the one thread that looks at the latest file asked for (`ask(input, reply)`), a thread per peek with an 8 MiB stack, the 4 s overrun and the cap on abandoned peeks; it takes no async runtime |
| `when` | `modified_text`: a modification time as the Info panel words it, in the person's zone |
| `pane` | `Pane`, `Part`, `Parts` (which of the media, the name and the facts it draws; all by default), `STYLE`; `page` (a `PageLook` to and from quire's `PdfPage`, the only place the two meet), `picture` (a `TextureLayer`), `lines` (plain and highlighted), `grid` (a table, a tree's top level and an archive's first entries, all as quire's `Table`), `specimen` (a font's sample lines, each an inline SVG of the face's outlines) and `frame` (Markdown in a sealed frame) are private, and `pane.css` is its stylesheet |

`peek(src, sniffed, budget)` never fails: a peek that cannot be made returns a `Body::Unavailable` with the
reason and the facts the file can still give. The pane draws a `Body` over the file's name and a `FactList`
of its facts; the actions under it are the host's.

## 2g. Modules inside `anyview-pdf`

Same rules as section 2. Blocking and effect-free except the jobs of `job`, which the binary's pool
runs; the crate spawns nothing, reads no file but the one it is asked to open, and draws only to
CPU memory.

| Module | Holds |
| --- | --- |
| `error` | `PdfError` |
| `halt` | where a `Stop` meets pdfrum: its flag is the one pdfrum polls, its deadline is pdfrum's budget on it (private) |
| `document` | `PdfDocument` (open, `Send + Sync`, shared in an `Arc`), `DocId` |
| `geometry` | `MilliPoints`, `PageSize`, `PageRect` (a rectangle as thousandths of the displayed page) and `displayed`, the one place page space becomes displayed space (crop box and `/Rotate`) |
| `layout` | `PageLayout`, `PagePlace`: pages stacked at one scale, a height to a page and back |
| `tile` | the pure scheduler: `ZoomBucket` (four steps to the doubling), `TileKey` (page, bucket, column, row), `schedule`, `Schedule`, `TileBatch`, `ViewWindow`; `key` and `zoom` hold the tile arithmetic |
| `render` | `PdfWorker` (pdfrum's caches and the last prepared page, bound to one document), `Raster` (premultiplied RGBA8), `Tile`, `End`, and the two draws (private fns) |
| `search` | `SearchQuery`, `Hit`, `Hits`, `search_page`, `search_document` |
| `outline` | `OutlineEntry`, `Disclosure`, `outline`; `PdfLink`, `LinkTarget`, `page_links` |
| `edit` | `PageOp`, `page_op` (a core `Edit` to a page edit), `apply` (edits to the bytes of a new file) |
| `export` | `plan_export` (a PDF choice as `ExportJob`s: one for a PDF or a text file, one for each page of page images), `selected`, `write_pages`, `write_text` |
| `pictures` | `PagePicture` (a JPEG kept as it is, upright pixels, or an SVG drawn as vector paths) and `pdf_of_pictures`: one page for each, the size of its picture and at most an A4 sheet's long side |
| `bind` | `Bookmark`, `bind`: PDFs joined in order into one, with an outline whose lines go to the first page of a part, or to the first page that shows their words (how the viewer binds a book) |
| `info` | `PdfDocument::info` and `PdfInfo`: the Info dictionary (or the XMP packet when it lacks a title, author, subject or keywords), the version, the page size named (`A4`, `Letter`, `210 × 297 mm`, `Varies`), pages, protection and what an encrypted file takes away, tagging, attachments and signatures; `PdfInfo::facts` are the Document section's rows. PDF dates are parsed in `date`, XMP read with `roxmltree` in `xmp` |
| `job` | `PdfBackend` (implements `Backend`), `PdfJob`, `PdfDone` |

A view asks `schedule` for the tiles it needs (the visible ones nearest the middle first, then a margin
all round), takes `Schedule::batches` as jobs (one per page; the worker keeps the page it read, so the next batch of that page at that zoom does not read it again),
and runs each with `PdfBackend::run`. Tiles are drawn at a zoom *bucket*, the lowest step of the ladder
that is at least the shown scale, so a pinch redraws at a few scales, not every frame; a tile is the
same pixels as that region of a whole-page render at the bucket's scale. Every `PdfDone` carries the
`Ticket` of its job, and the receiver drops one that is not the latest. A `Stop` ends a job: its flag is
the one pdfrum polls, so a raise ends a draw between the drawn objects of a tile (`End::Stopped`), and
a deadline is pdfrum's budget on the same flag. A stopped job keeps what it finished.

Pixels are premultiplied RGBA8, the form `TextureLayer` uploads; `Raster::straight_rgba` is the form an
encoder takes. A page is drawn on white. Hits, links and outline entries carry pages, never object
numbers, and rectangles are fractions of the displayed page, so they need no zoom.

## 2h. Modules inside `anyview` (the runtime)

`lib.rs` declares `pub mod runtime`; `main.rs` is the program. The runtime is the one place that
starts threads (section 5b), and it names no back end: it is generic over `anyview_core::work::Backend`
and over an actor body.

| Module | Holds |
| --- | --- |
| `runtime::error` | `RuntimeError` (`Spawn`, `ActorEnded`) |
| `runtime::mailbox` | `UiWaker`, `Mailbox` (the UI thread's end, `drain`), `Outbox` (cloned to every posting thread; one wake per drain) |
| `runtime::pool` | `Pool`, `PoolSize`, `Lane` (`Visible`, `Preload`); the per-worker scratch map and the queues are private |
| `runtime::runner` | `Runner<B, T>`, `JobHandle`, `JobOutcome` (`Done`, `Skipped`, `Panicked`), `JobPanic` |
| `runtime::actor` | `Actor`, `ActorBody`, `ActorWake`, `Flow` |

The program is the rest of the library. `main.rs` reads the process's arguments, working directory and
environment once and calls `program::run`; nothing below it reads `std::env`.

| Module | Holds |
| --- | --- |
| `cli` | `parse`, `Invocation` (`Help`, or a `Launch` of the platform's `Request`), `CliError`, `USAGE`: the arguments as the request a launch makes |
| `program` | `run`; `claim_role` and `Role` (`Forwarded`, `Primary`, `Alone`: single instance over the `Instance` trait); `relay`, `open_each`, `open_windows`, `wants_of`, `Want` and `Arrival` (what the viewer's name receives: each file, or a handoff that brings its own results and place, becomes a window through ds-blitz's `AppHandle`, or, for a `Play`, a player with no window); `WARM_FOR` |
| `media` | the program's players: `MediaHub` (the sessions, the desktop's one now-playing entry and the controls that come back, the sessions with no window), `PlayerHost` (the `MediaHost` a window is lent: a player on a thread of its own per window), `NowPlaying` (MPRIS, or absent without a bus), `MediaPlugins` (the registry and the runner: `player`, `reading`, `writer`, `offer`; section 2i), `Exports`, `ExportHandle`, `ExportEnd`, `PluginExport` (the pool's runner for transcodes through the FFmpeg plugin, with progress and stop). Private: `actor` (the `ActorBody` over a `Box<dyn MediaDriver>`), `engine` (which player a recording gets, `choose`, and what each is built from), `guard` (`Guarded`: a panic in a player becomes it failing), `line` (what a window holds of a player), `map` (the player's events and commands to the machine's, both ways), `snapshot` (the entry's state from the events, and how often a moving position is published), `orders` (what each desktop control means to a player), `sink` (the window's texture as the player's picture) |
| `seam` | `Workforce`: the `Pool`, the `Runner` for the views' `Work` and the `Mailbox` its endings come back through; `NoticeWaker`, `Notice`. The one implementation of `anyview_ui::Workers` |
| `host` | `route` (a `HostRequest` as a `Carry`: the window's own `WindowTask`, the desktop's `Task`, or a `Declined` with its reason; pure), `Shown` (the file a window shows), `Desktop` and the `Hosting` trait (the tasks carried out through the platform's traits), `PlatformDesktop` (the desktop of this build: the Linux services with `quire-desktop`, else the portable stand-ins), `Trash` with `SystemTrash`, `Store` (the one writer of the history, behind a lock) and the `Clock`, `Remembering` (the places waiting to be written, at most every `REMEMBER_EVERY`), `Watcher` and `WindowWatch` (the one file watcher and each window's end of it), `HostedResume`, `HandedResume` (the place a handoff held, read once for its file before the store's) and `CachedPictures` (the store and the thumbnail cache as the views' `ResumeSource` and `StillSource`), `Outcome`, `Declined` and `feedback` (the words the person is told of each outcome, `notice_of`, and the program's one log line, `log`: every task's end goes through `tell`, which logs it and hands the window its `Notice` through `Edge::notify`, which the window draws as a toast, with Show in Folder when the notice names a file), `StoreLocks` (the store's read-only check as the views' `FileLocks`), `Media` (the hub, the exports and a scratch folder) and its two tasks: play with no window from where the file was left, and write a media export beside the file (a cut or a track on the pool, the frame on screen from the player that shows it); `helpers` (`HelperHost`: the tools the plugins run and a missing one's install, below), `plugin_registry` (`PluginRegistry`: the plugins as they are now, read again when a tool is installed), `path_watch` (`PathWatch`: the folders of the search path, watched so a tool installed in a terminal is noticed), `documents` (the export of an image, a PDF or a text document through `anyview-export` on the blocking pool, and the PDF `Print` hands the printer for any file that prints) |
| `window` | `fit` (where a viewer window's size is decided, below), `Opening` (a file, its sequence (its folder's, or the results a handoff brought) and the place a handoff held), `Factory` and `Seed` (what every window shares, and what makes one window its own), `open_in_window` (a window opened through the `AppHandle` with its `Seed` as props) and `seeded_root` (a root that reads the `Seed` from a context: the harness's) |

**Where a window's size is decided.** `window::fit` is the one place, in two steps. As a window opens, `spec_for`
(from `open_in_window`, with `AppHandle::screen_area()`) asks `window_for(file, screen)`, which reads the file's
natural size with `anyview_peek::natural_size` (a few milliseconds, bounded, nothing decoded) in device pixels and
turns it into logical ones with `ScreenArea::logical_for_pixels` (one image pixel to one device pixel, as Preview
shows a picture: a 1200 by 800 picture is a 600 by 400 window on a 2x output). The pure `fitted(natural, work,
least)` then fits it with quire's `WindowSize::fitting_with(.., Fit::KeepRatio)`: scaled down to 85% of `work`
keeping its ratio, never below `LEAST` (480 by 320, kept as the window's least size too), and `WINDOW` (1000 by
700, held to the cap per axis) for a file with no natural size. `work` is the screen's work area less the top bar
(`TOP_BAR`, a `Reserve` of 32 logical pixels at the top: Wayland reports no panel to a client, so quire's work area
is the whole output), the fixed `DEFAULT_SCREEN` (1920 by 1200) before the event loop runs (the very first
window), or `ANYVIEW_WINDOW_SCREEN=WIDTHxHEIGHT` (for tests). A PDF's page is `NaturalSize::Points`: a point is a
logical pixel on any screen, so it is not divided by the scale.

Every different file sizes the window, as Preview does. When a file's full open has landed, the window tells the host
what to size to as `HostRequest::SizeWindow(SizeBasis)`: `Natural` when the document knows (`StageView::natural`: a
PDF's first page as displayed, at 100%, from pdfrum's crop box and rotation; a picture's decoded size, which a
plugin's HEIC or camera raw only has then), `Header` when it knows none (a recording's picture, a picture whose
plugin is missing), and `Default` when the file failed to load, so that it is sized as any file with no natural
size and not left at the last file's size. The shelf's `sizing` ticket is set by each different file's `Open`
and spent by the first of its landing or failing, so a reload of the same file, and a file the person has left,
send nothing. The window routes it to `WindowTask::Size`, which reads a `Header` with
`anyview_peek::natural_size` on a blocking thread (a video's is a probe of the file; never the UI thread) and drops
the answer if the window has moved to another file by then. `WindowFit::loaded` then calls quire's
`WindowSizer::request_size(fitted)` unless the window is maximized or fullscreen (its size is the desktop's, so
a request would only fight it; `WindowState` has no tiled state, see FINDINGS), or already is that size. The size a
person gave a window does not hold against the next file: Preview and Photos size each document, and so does
this. It fits to `WindowSizer::screen()`, which is exact once the window is mapped (its own output and fractional
scale), so a first window that opened before the screen was known is corrected here. A request nobody answers
expires in quire after 500 ms and is not retried. A test runs the harness's window (`SizerAck`, `WindowScreen`,
`Harness::window_requests`). The mini window is never resized. The mini and welcome windows keep their own constants.

A wheel's detents reach the PDF, picture and text views as `WheelDelivery::Eased` gestures: one
`Gesture::Scroll` per frame whose shares sum to 60 px a detent over at most 200 ms (design/11 §11.3.11). A touchpad's
run reaches the same listener as `Began`, `Changed`, the glide after a fast lift (one `Changed` a frame) and
`Ended` (`ScrollSource::Finger`), all quire's; the views apply `by` as they do a share and clamp at the edges (no
rubber band). Under Control a wheel reaches every listener raw, one 60 px gesture per click, and the PDF view zooms
by it. The PDF view carries the part of a share under a device pixel to the
next frame. Table, tree, the failure and welcome screens and the PDF panel are native overflow or quire's
`VirtualList`, which the window scrolls by design/11 with no code of ours.

A window's `HostRequest`s go from its `Edge` over a channel to a task of its root component, which routes each
and either does it itself (closing the window, the clipboard: only that thread can) or hands the task to
`Hosting::carry_out`, which runs it on the platform runtime and resolves with an `Outcome`. A second launch is
claimed by the first viewer over D-Bus (or, without `quire-desktop`, over the per-user socket); its files become `Opening`s (the folder is listed on the blocking pool),
which a task of the platform runtime turns into `AppHandle::open_window_with` calls on the one event loop (a `Play` has no window: its arrival is a player with none). A window cannot change its own frame, so becoming the small borderless window of a recording (or a window again) is a window made again for the same file, opened with a `Seed` that says its `Presentation`, after the place the old one was at is written (`WindowTask::Reopen`). Every
window is opened that way, the first included: the loop starts with none (`launch_idle`), windows are independent,
and the last one closing leaves the process warm for `WARM_FOR` (`LastWindowClosed::StayFor`).

## 2i. Modules inside `anyview-media`

Same rules as section 2: private modules, each public item re-exported once at the crate root. The crate
links no libmpv and no libav. Playing is the person's own `mpv`, run as a child process with mpv-wgpu's C
plugin loaded into it (`mpv-wgpu-player`'s `subprocess` host, section 2m); the feature `player` is that and
the GPU texture the picture is drawn into. Writing a recording is the FFmpeg plugin's (section 2l): this crate
only plans the work, names the file and says what to ask. The launcher's pane links none of it: it reads
recordings with pure-Rust parsers (section 2f). Nothing here spawns a thread of its own, reads a clock or draws a
pixel: the binary runs the driver on the media thread and the exports on its pool. With no mpv, audio plays in
the crate's built-in player (feature `audio`), which the binary chooses per recording (`media/engine.rs`):
mpv when a plugin has it, else the built-in player when it decodes the file and the machine has a sound
output, else the facts card with the row that names what would play it (`anyview-mpv`, or a sound output).

| Module | Holds |
| --- | --- |
| `error` | `MediaError`, the crate's one error: the player (`Player`, and the typed `PlayerGone`, `PlayerSilent`, `PlayerStart` and `PlayerMissing` of the child process), the built-in player's (`Decode`, `NoDecoder`, `NoSoundOutput`, `SoundOutput`), `Export`, `WriterMissing`, `Stopped`, `NotMedia` |
| `command` | `MediaCommand` (what the viewer tells the player), `Pace`, `Direction`, `PictureSlot`, `ShotContent` |
| `event` | `MediaEvent` (what the player says, including `Length`, the length a recording gives after it is loaded), `EndReason` |
| `session` (feature `player`) | the typestate: `Session<Idle>` (made on a device and queue and an `MpvHost`, given a file with `open`), `Session<Opening>` (`poll` says whether it opened or gave up: `Opened`), `Session<Loaded>` (the only one that has `tracks`, `chapters`, `seek`, `set_volume`, `set_speed`, `select_track`, `cycle_track`, `step_chapter`, `frame_step`, `screenshot_to_file`), `MpvHost` (the `mpv` and the C plugin, from a `play` entry of a plugin manifest), `Report`, `Frame`, `AudioDriver`, `Refused`. Moving between the states consumes the session. `convert` is where the player's words become the viewer's (private) |
| `driver` (feature `player`) | `Driver` (one session and the state around it: instructions that arrive before the file opens wait and run in order when it does, a position at most ten times a second and held back while a seek is in flight, the length said once it is known, the end of a file mpv holds open reported as `Ended`), `FrameSink` (where the picture goes: a new texture, a new frame, none) |
| `playing` | `MediaDriver`, the trait the media thread's actor holds a player behind (`command` and `woken`; two implementations, mpv's `Driver` and `BuiltinDriver`, and the actor's `Guarded` wraps either), `Handled`, `Continuation` |
| `builtin` (feature `audio`) | `BuiltinDriver` (the built-in player as a `MediaDriver`: opens the file, decodes ahead of the card, says `Loaded`, the position ten times a second, `SeekDone` and the end, and holds what is sent before it is loaded), `playable` (whether a file decodes: its codec has a decoder and its first buffer decodes; a picture or Opus does not), `SoundOutput` (the seam the card is behind: `negotiate`, `start`, `pause`, `resume`) with `CardOutput` (cpal's default device), `SilentOutput` (a clock thread that plays nothing, for `ANYVIEW_AUDIO_OUTPUT=null`) and `card_present`, `Pipe` (the queue between decoder and card, whose drained frames are the clock) with `Gate`, `StreamFormat`. Private: `source` (symphonia: probe, decode, accurate seek, with a replay from the start for a container that cannot seek), `convert` (channel mixing and a streaming linear resampler to the card's format) |
| `device` (feature `player`) | `headless_device`: the device a session with no window plays on |
| `export` | `plan_export` (a choice as `ExportJob`s: pure), `ask_of` (what a job asks of the plugin: its target, range and bitrate), `target_of` and `offered_kinds` (which target writes a kind, and which kinds fit a file's kind), `output_path` with `NameHints` (a free name beside the source), `ExportRequest`, `ExportProgress`, `ExportReport`, `ProgressSink`; `ask`, `naming`, `plan` and `request` are private |

The session is `Send` and not `Sync`, and its `poll` is called by whoever owns it: the media thread, which
is not the thread that presents (FINDINGS, "`Player::poll` works from a thread that does not present").
**What the child process changes.** Every call that reaches mpv is a round trip over a socket; the reads of
tracks, chapters, the position and the length are caches that mpv's events keep. mpv reports the length
and the chapters a moment after it says the file is loaded, so `Loaded` may carry no length and the driver
says `Length` when it arrives (the stage keeps it), a chapter instruction waits for the chapter list, and the
end of a held file is looked for once a poll's last position is in. A crashed or hung mpv is `PlayerGone` or
`PlayerSilent`: a `Failed` event, which the stage shows as "The player stopped"; a new session is how to play
again. The frame on screen is mpv's: `ExportJob::MpvScreenshot` is planned here and carried out by the player
that shows the frame (`MediaCommand::Screenshot`).

**The binary's side (`anyview::media`).** `MediaPlugins` (the registry from `discover(&Env)` and the
`PluginRunner`) answers four questions: `player` (the `MpvHost` of a kind, else the package that would play it),
`reading` (a recording's facts and tags: the FFmpeg plugin's, else `anyview-peek`'s pure-Rust ones), `writer`
(the plugin that writes exports) and `offer` (which media exports the sheet lists). `offer` asks the writer's
greeting which targets this machine's FFmpeg can encode and keeps those that fit the file's kind (a frame is
a video's, and the player's, so it is on offer only with a player). With no plugin the viewer shows what it
can: a recording nothing plays opens as its facts with a `Needs: anyview-mpv` row, and an export sheet with
nothing to offer says which package adds it. `Exports` runs `PluginExport`, a pool job whose worker waits
for the plugin, reports its progress and cancels it when stopped (the plugin removes what it wrote).

## 2j. Modules inside `anyview-archive`

Same rules as section 2. Blocking and effect-free except the reads of the one file it is asked about.
A listing reads inside a byte budget: a zip or a 7z whose index is larger than it is refused
(`ArchiveError::OverBudget`), a tar or a compressed stream is listed as far as the budget reaches and
its count is a lower bound (`EntryCount::AtLeast`).

| Module | Holds |
| --- | --- |
| `error` | `ArchiveError` |
| `container` | `container`, the one match on `ArchiveFormat`: `Container` (`Zip`, `Tar`, `Compressed(Codec)`, `SevenZip`) and `Codec`; `format_name` and `kind_name`, the words for a format (private) |
| `entry` | `Entry`, `EntryKind`, `EntryCount`, `EntryLimit`, `Holds`, `Listing`; `Tally`, which keeps the first entries and counts the rest (private) |
| `limit` | `Limited` (a reader that stops at the budget and says so) (private) |
| `list` | `list`: the one entry point, and the compressed-stream case (a tar inside, or one file) |
| `extract` | `extract`, `ExtractLimits`: one entry's bytes, bounded by the entry and by how much of a stream is unpacked |
| `zip_archive`, `tar`, `sevenz`, `stream` | one container each: the central directory (and `zip_entries`, the names and `mimetype` sniffing wants), tar headers (also over a decompressed prefix), the 7z header (`sevenz_header` checks every size it states against the file, a count limit and a byte limit before the 7z crate allocates from it), and gzip, bzip2, xz and Zstandard unpacked up to a cap through readers that stream (private except `zip_entries`) |
| `peek` | `ArchivePeek`, `ArchivePeeked`, `PEEK_ENTRIES` |

A compressed stream is a tar when what it unpacks to parses as one (the header checksum decides), else
one file named like the stream without its last extension. Everything is held in memory up to the cap:
a listing's cap is the peek budget, an extraction's is `ExtractLimits::scanned`.

## 2k. Modules inside `anyview-font`

Same rules as section 2. Blocking: the peek reads the one file whole, which must fit the byte budget,
because a font's tables lie all over it.

| Module | Holds |
| --- | --- |
| `error` | `FontError` |
| `face` | `Face`, `Variation`: the family and style names, glyph count and variation axes of a face, read with skrifa |
| `specimen` | `Specimen`, `SpecimenLine`, `EM`: sample lines set with each glyph's advance (no shaping, no kerning) as SVG path data in 1000 units to the em |
| `woff` | `to_sfnt`: a WOFF's tables unpacked and laid out again as the plain font it wraps (private) |
| `peek` | `FontPeek`, `FontPeeked` |

The specimen is outlines, not a font handed to the renderer: the pane needs no font loading, the peek
stays a small value whatever the font's size, and what is drawn is the face the file holds. A font that
maps none of the sample letters shows the first characters it does map. A WOFF is unpacked to the
plain font it wraps (`woff.rs`: each table's zlib stream, the tables laid out again as an sfnt; the
tables together may unpack to 64 MiB and may not share stored bytes) and read
like one; WOFF2 is named (`face: None`) and not opened.

## 2l. Plugins

Codecs, and every other piece of code that is copyleft or patent-encumbered, are not linked. A plugin
is a separate executable the person installs, which uses their own distribution's mpv and FFmpeg and
talks to the viewer over a pipe. The viewer asks it to do one thing and kills it when it is done, so a
plugin that crashes, hangs or lies costs one request and never the viewer.

bayonet, a repo of its own (`github.com/PoHsuanLai/bayonet`, MIT OR Apache-2.0, used by other apps too), carries
the mechanics that do not depend on what a capability is: the wire's frames, the manifest's envelope, discovery in
`<data dir>/<app>/plugins`, the ranking of plugins, the package suggestion, and starting, timing out, cancelling
and killing a plugin. It is generic over the capability type; the viewer's capabilities and their messages stay
here, implemented on bayonet's two seams, `Provides` (one `[[provides]]` entry) and `Protocol` (the messages and the
greeting). Three crates, a test plugin and the FFmpeg plugin carry the rest:

| Crate | Holds |
| --- | --- |
| `anyview-plugin-protocol` (L0) | `Capability`, `HostMessage`, `PluginMessage` and the request and reply types, `PROTOCOL_VERSION`, `ImageSizeError`, and bayonet's frame codec re-exported (`encode_frame`, `read_frame`, `write_frame`, `FrameDecoder`, `WireError`). Pure: bayonet's wire, `serde`, `thiserror` |
| `anyview-plugin` (L1) | `Provision` (`Probe`, `Peek`, `Thumbnail`, `Decode`, `Export`, `Play`: bayonet's `Provides` for the viewer), `Handles`, `Subject`, `Plugins` (the registry over bayonet's), `Route`, `MissingPlugin`, `suggested_package`, `ProvisionFault`; and as bayonet's generic types fixed to the viewer's: `Manifest`, `Candidate`, `Installed`, `Unusable`, `Readiness`, `Package`, `PluginError`; `PluginId`, `Program`, `Origin` as they are. Pure |
| `anyview-platform` (L2), module `plugin` | `discover(&Env)`, `PluginRunner` (`probe`, `thumbnail`, `decode`, `export`, and `hello`: what the plugin answers on this machine, which a host reads the export targets it can write from), `Timeouts`, `PluginFacts`, the private `Wire` |
| `anyview-ffmpeg` (`plugins/`) | `anyview-ffmpeg`, the FFmpeg plugin (below) |
| `anyview-plugin-fake` (dev) | `anyview-fake-plugin`, a plugin for one invented kind that can misbehave on request, and the tests |

The viewer's part of the manifest lives in `anyview-plugin`, apart from `anyview-core`, so that the core stays free of a TOML
parser and the protocol crate free of the viewer's vocabulary (a plugin author needs the messages and
nothing else); the registry is pure so that crates above the platform, which cannot name it, can still be
handed a `Plugins` value.

### The manifest

A plugin installs one file, `<id>.toml`, in `anyview/plugins/` under a data directory:
`$XDG_DATA_HOME` (the person's) and each of `$XDG_DATA_DIRS` (the system's). The directories come from
`Env`; a test builds its own. A file that cannot be read, is not a manifest, or is not named for its id is
reported (`Discovery::rejected`) and skipped.

```toml
id = "ffmpeg"                  # [a-z0-9_-], up to 64; must equal the file name without ".toml"
name = "FFmpeg"                # for people
protocol = 1                   # the newest protocol version the program speaks

[program]                      # what speaks the protocol; omit for a plugin that only provides play
path = "/usr/libexec/anyview/anyview-ffmpeg"   # absolute
args = ["--serve"]             # placed before anything the host adds (nothing in v1)

[[provides]]                   # one entry for each capability, at most once each
capability = "probe"           # probe | peek | thumbnail | decode | export | play
kinds = ["video", "audio"]     # FormatKind slugs (anyview-core)
mimes = ["video/x-extra"]      # optional; kinds and mimes may not both be empty

[[provides]]
capability = "export"
kinds = ["audio"]
targets = ["mp3", "flac"]      # required for export, rejected elsewhere: [a-z0-9_.-], up to 32

[[provides]]
capability = "play"            # not spoken over this protocol: mpv-wgpu starts the player from these
kinds = ["video", "audio"]
mpv = "/usr/bin/mpv"           # absolute; the stock player
cplugin = "/usr/lib/anyview/mpv-wgpu-cplugin.so"   # absolute; loaded into it with --script
```

Unknown keys are ignored. Every capability except `play` needs a `[program]`; `play` needs both `mpv`
and `cplugin` and the others may not carry them. `peek` is not a request of its own: the host serves it by
asking `probe` and then `thumbnail`, so a plugin that lists `peek` answers both.

Discovery then checks every path the manifest names: the program and `mpv` must exist and be executable,
the C plugin must exist. A manifest that fails is `Unusable` with the reason and takes part in nothing.

Precedence, applied by `Plugins::resolve` and never dependent on the order the disk lists files in:

1. A manifest whose protocol is newer than the viewer's (`PROTOCOL_VERSION`), or whose programs are
   unusable, is set aside first, so a broken copy never shadows a working one.
2. Of the manifests with one id, the higher protocol version wins; at equal versions the person's
   directory wins over the system's; between system directories the earlier in `$XDG_DATA_DIRS` wins.
3. Plugins with different ids are tried in the same order (protocol, then the person's first), then by id.
   `serving(capability, subject)` picks the first that provides the capability and lists the subject's kind
   or MIME type, preferring one that lists the MIME type itself.

`Plugins::route` turns a request into `Route::Served(plugin)`, `Route::Missing(MissingPlugin)` (no plugin
serves it, and the static table in `missing.rs` names a package: `anyview-ffmpeg` for probing, peeking,
thumbnails, frames and exports of video and audio, `anyview-mpv` for playing them, and by media type
`anyview-heif` for `image/heic` and `image/avif` and `anyview-raw` for `image/x-dcraw`: a PNG needs no plugin and
is `Unserved`) or `Route::Unserved`.
`MissingPlugin::fact` is the `Needs` row a facts card lists. `export_targets` lists what an export sheet
may offer: each target of each installed plugin that exports the kind, once.

### Protocol version 1

**Process model.** The host starts the program once for each request and kills it when the call returns
or is dropped. The plugin reads one request, answers it, and exits. This costs a process start for each
request, which is nothing next to what ffprobe or ffmpeg cost, and in return a plugin holds no state, a
leak or a stuck codec cannot outlive its request, and cancelling is killing. A later version that keeps
a plugin alive for a session adds request ids and says so in `Hello`; version 1 has none, since only one
request is ever open.

**Streams.** The plugin's stdin carries host messages, its stdout carries plugin messages and nothing
else, and each line it writes to stderr goes to the viewer's log, prefixed `anyview: plugin <id>:`. The
last lines also ride in the error when a plugin dies. A line is kept to 4 KiB (the rest is dropped), and stderr
is drained when a request is sent as well as while the host waits, so a chatty plugin never blocks on a full pipe.

**Frame.** Every message is one frame:

```text
u32 little-endian: length of the JSON   (at most 1 MiB)
u32 little-endian: length of the payload (at most 512 MiB; zero for most messages)
JSON of the message
payload bytes
```

The JSON is adjacently tagged: `{"kind":"probe","v":{"path":"/a/b.mkv"}}`; a message with no fields is
`{"kind":"cancel"}`. Keys the reader does not know are ignored, and optional fields have defaults, so
fields can be added without a version. A plugin written in Rust uses `read_frame` and `write_frame`.

**Handshake.** The plugin speaks first:

```json
{"kind":"hello","v":{"protocol":1,"name":"ffmpeg","provides":["probe","thumbnail","decode","export"],"targets":["trim","mp3"]}}
```

`targets` is optional (empty when absent): the export targets the plugin can write on this machine, which may
be fewer than its manifest lists, since a distribution's FFmpeg may lack an encoder. A plugin that is asked
for a target it cannot write answers `unsupported`. A plugin that cannot do its work at all (no FFmpeg) says
hello with an empty `provides`, logs why on stderr and exits, and the host reports `PluginLacks`.

The host checks that `protocol` is the one it speaks (`PluginVersion` otherwise) and that `provides` lists
the capability it is about to ask for (`PluginLacks`), then sends the request. No `hello` within
`Timeouts::hello` (5 s) is `PluginSilent`.

**Requests** (host to plugin; paths are absolute):

| Message | Fields | Answer |
| --- | --- | --- |
| `probe` | `path` | `facts` |
| `thumbnail` | `path`, `max_edge` (pixels on the longer side) | `image`, longer side at most `max_edge` |
| `decode` | `path`, `max_area` (pixels) | `image`, width times height at most `max_area`; the plugin scales down to fit |
| `export` | `input`, `output`, `target`, `range` (`{start, end}` in microseconds, optional; an absent `end` is the end of the recording), `stream` (index, optional), `bitrate` (bits a second of a lossy target, optional) | any number of `progress`, then `done` or `error` |
| `cancel` | none | sent only during an export: the plugin removes its partial output and answers `error` with `code` `cancelled` |

**Replies** (plugin to host):

| Message | Fields | Meaning |
| --- | --- | --- |
| `facts` | `rows`: list of `{label, value}` | `label` is a `FactLabel` slug (`kind`, `title`, `codec`, `duration`...); an unknown label is dropped; `value` is text already formatted for a person |
| `image` | `width`, `height`; payload | the payload is exactly `width * height * 4` bytes of straight (not premultiplied) RGBA8, rows from the top, already upright |
| `progress` | `done`, `total` | any unit; `total` 0 when unknown |
| `done` | `output` (optional) | the export is finished; the plugin writes `output` completely before saying so |
| `error` | `code`, `message` | `code` is `unsupported`, `unreadable`, `corrupt`, `too_large`, `cancelled` or `failed`; the plugin then exits |

**Pictures** cross as raw RGBA8 in the frame's payload, on the same pipe. A 24-megapixel decode is 96 MB,
which a pipe moves in tens of milliseconds, against about a second to encode and decode it as PNG; there is
no side socket, no file descriptor passing and no `unsafe` on either side, and a plugin in any language can
write it. The host checks the payload against the header and the header against the budget it asked for
(a plugin that sends more than `max_edge` or `max_area` is `PluginProtocol`), and no payload may exceed
512 MiB. A reply that may carry a picture is held to the bytes of the pixels asked for: a header that
announces more is refused before any of the payload is buffered.

**Timeouts, cancel and drop.** Waiting is by `poll` on the pipe, so no thread is needed. A plugin may be
silent for at most `Timeouts::silence` (30 s) while a request is open; each message starts the wait again,
so a long export that reports progress is never cut off. When an export's `Stop` is raised the host sends
`cancel` and gives the plugin `Timeouts::cancel_grace` (2 s) to answer; after that it is killed. The result
is `PluginCancelled` either way, unless the export finished first. Dropping a call kills the process and
reaps it.

**Failures** are `PlatformError` variants a caller acts on, and none of them panics or ends the viewer:
`Spawn` (the program cannot start), `PluginSilent`, `PluginCrashed` (the output ended; carries the exit
status and the last stderr line), `PluginProtocol` (not a frame, a message out of turn, a picture the wrong
size or over budget), `PluginVersion`, `PluginLacks`, `PluginFailed` (the plugin's own `error`, with its
code) and `PluginCancelled`.

**Playback is not on this protocol.** `play` is a capability whose manifest names the `mpv` and the C
plugin; mpv-wgpu's player starts them and keeps its own frame protocol with its C plugin inside mpv-wgpu.

### The routing seam

Where the viewer chooses a back end for a kind, it asks the plugins when no built-in back end handles the
kind or capability. `PluginRunner::peek_facts(&Plugins, &Subject, &FilePath)` is that call for facts: a
plugin that serves `probe` for the kind is run and its rows become `Facts`; otherwise the answer is
`PluginFacts::Missing(MissingPlugin)`, whose `fact()` the card lists, or `PluginFacts::Unserved`.
Thumbnails and decodes are `PluginRunner::thumbnail` and `decode` after `Plugins::route(Capability::Thumbnail
or Decode, ..)`, and exports `PluginRunner::export` after `export_targets`. Like every blocking call these
run on a worker. Nothing in the viewer calls them yet: the peek registry and the stages still name their
built-in back ends (FINDINGS, "Plugins are not wired into the viewer").

### Writing a plugin

1. Depend on `anyview-plugin-protocol`.
2. In `main`, write a `hello` frame, read one frame, answer it as above, and exit. Read stdin on a
   thread of its own if the plugin must notice `cancel` during an export.
3. Install the program (the FFmpeg plugin's goes in `<prefix>/libexec/anyview/`), and `<id>.toml` next to the
   others in `<prefix>/share/anyview/plugins/`.
4. Write nothing to stdout but frames; log to stderr.

`crates/anyview-plugin-fake/src` is a complete example, and its tests drive it through the host.

**A plugin's process group.** The host starts a plugin in a process group of its own and, when the call ends
or is dropped, kills the group, so a program the plugin started (ffmpeg) dies with it even when the plugin
is killed and cannot clean up. A plugin that cancels still kills its own children and removes its partial
output before it answers.

### The FFmpeg plugin

`plugins/anyview-ffmpeg` (binary `anyview-ffmpeg`, package name `anyview-ffmpeg`) is the plugin the viewer
probes and exports recordings with. It lives in `plugins/` and not
in `crates/` because it is a program with its own package and its own licence story, not a layer of the
viewer: `crates/` holds what the viewer links, `plugins/` what it runs. It is the one place in the repository
that reads the environment and starts programs on its own account, since that is what a plugin is. Its
only edge is `anyview-plugin-protocol` (plus `serde`, `serde_json`, `thiserror`); `cargo tree` for it holds
no `ffmpeg-next`, `ffmpeg-sys-next` or `rsmpv`, and `check-boundary` says so for its dependency tree.

**Finding the tools.** `ffmpeg` and `ffprobe` are each found as an absolute path in the manifest's
`args` (`--ffmpeg PATH`, `--ffprobe PATH`), else `ANYVIEW_FFMPEG` and `ANYVIEW_FFPROBE`, else the first
match on `PATH`. A path that is named and is not an executable file is an error, never a reason to look
further. At startup it runs `ffmpeg -version` (FFmpeg 4 or newer, or a development build) and
`ffmpeg -hide_banner -encoders` once, so `hello` lists only the targets this machine can encode. With no
FFmpeg `hello` lists nothing and stderr says why.

**Facts.** `ffprobe -v error -print_format json -show_format -show_streams -show_chapters`, JSON only.
The rows, in order, each only when the file has it: `duration`, `dimensions` and `codec` (the first
moving picture), `framerate`, `audio-codec` (or `codec` for a recording with no picture), `sample-rate`,
`channels`, `bitrate` (the sound's, else the container's), `title`, `author` (the artist tag), `album`
(tags found in the container then the streams, whatever the case of their keys), `streams`
(`1 video, 2 audio, 1 subtitle, 1 cover`, when there is more than one) and `chapters`. Facts the viewer
fills itself (`kind`, `size`, `modified`) are not sent.

**Pictures.** `thumbnail` and `decode` run `ffmpeg -ss T -i FILE -map 0:N -frames:v 1 -vf scale=W:H,setsar=1
-pix_fmt rgba -f rawvideo pipe:1` and send the bytes as the `image` payload. The size is worked out from the
probe (pixel aspect applied, a quarter turn swapping the sides, never enlarged), so the reply is exactly what
the host's budget allows. `T` is a tenth of the duration, at most five seconds, and falls back to the first
frame for a recording too short for it. A sound shows its attached cover; one with none answers `unsupported`.

**Export targets**, spelled as the manifest spells them:

| Target | Writes | How |
| --- | --- | --- |
| `trim` | the recording cut to `range`, in the container the output's extension names | stream copy of every picture (not covers), sound and subtitle stream; chapters and tags kept |
| `audio-copy` | the audio track as it is | `-c copy` of the track FFmpeg would pick (most channels), or `stream` |
| `m4a`, `mp3`, `opus` | AAC, MP3 (LAME), Opus (libopus, else the native encoder) | `bitrate`, 192 kbit/s when absent; at most two channels; MP3 resamples to 44.1 kHz when LAME lacks the rate, Opus to 48 kHz |
| `flac`, `wav` | lossless FLAC, 16-bit PCM | the source's rate and channels |

All of them honour `range` (audio is cut to the sample) and `stream`. The viewer's own names for the
output files stay the viewer's: the host passes `output` and the extension it wants.

**Trim and the keyframe.** A cut of a recording with pictures can only begin where a picture does not depend on
an earlier one. The plugin finds the last video keyframe at or before `range.start` with
`ffprobe -select_streams INDEX -read_intervals ... -show_entries packet=pts_time,flags` (packets only, no
decoding; it looks 30 s back, then 10 minutes, then the whole file), and runs
`ffmpeg -ss KEY -i FILE -t END-KEY ... -c copy -copypriorss 0`. Seeking exactly to the keyframe and dropping
what precedes it (`-copypriorss 0`) makes the output begin on that keyframe at time zero, with every stream
aligned to it; without it a subtitle cue that began earlier pulls the
whole file's timeline back. `-ss` is given a microsecond before the keyframe so a rounded timestamp cannot
make FFmpeg drop the keyframe it was sent to find.

**Progress, cancel and the output.** Progress is `-progress pipe:1 -nostats`: each `out_time_us=` line is a
`progress` message with `total` the length written (zero when unknown); a last one reaches `total` before
`done`. ffmpeg writes to a hidden `.part-<pid>-<n>-<name>` beside the output (made new by the plugin, so it never
touches a file it did not make) and the file is given its name only when ffmpeg succeeded and left something, so
an export is whole or absent. It never overwrites: an existing `output` (a dangling symlink included) is an
error before anything runs, and a hard link claims the name at the end. The partial files of dead plugin
processes are removed at the next export. `cancel` (or the host closing
its pipe) kills ffmpeg, removes the partial file and answers `cancelled`; ffmpeg's last stderr lines ride in
the message when it fails.

**Install.** `dist/plugins/anyview-ffmpeg.toml.in` is the manifest template; `dist/install.sh`
installs the program as `<prefix>/libexec/anyview/anyview-ffmpeg` and the template, with `@PREFIX@`
filled in, as `<prefix>/share/anyview/plugins/ffmpeg.toml`; `uninstall.sh` removes both. `dev/install-test.sh`
covers it. The plugin tests spawn the built program through `PluginRunner` against the media crate's fixtures,
using the shipped template.

### The picture plugins

`plugins/anyview-heif` and `plugins/anyview-raw` (binaries and package names of the same names) make pictures
of what the viewer has no decoder for, over `plugins/anyview-tool-kit`. Each answers `decode` and `thumbnail`
with raw RGBA (the protocol's `Image` frame), says in `hello` what this machine has (nothing when its tools
are absent), and runs the person's own tools: it links no libheif and no LibRaw. Tools are found as a manifest
argument (`--heif-dec`, `--heif-thumbnailer`, `--dcraw-emu`, `--dcraw`), then an environment variable
(`ANYVIEW_HEIF_DEC`, `ANYVIEW_HEIF_THUMBNAILER`, `ANYVIEW_DCRAW_EMU`, `ANYVIEW_DCRAW`), then the search path;
a named tool that is absent is never replaced by another. A tool runs with a deadline and is killed when the
host sends `Cancel` or closes its pipe (the host also kills the plugin's process group when it gives up).

| Plugin | Tools | Fedora, Debian | What it relies on |
|---|---|---|---|
| `anyview-heif` | `heif-dec` (libheif 1.17 and later) or `heif-convert`, and `heif-thumbnailer` | `libheif-tools`, `libheif-examples` | `<tool> <input> <dir>/out.png`; the first of `out.png`, `out-1.png`… is the primary picture. libheif applies `irot` and `imir` by default, so the PNG is upright and nothing more is applied. A thumbnail is `heif-thumbnailer -s <edge>`, else a scaled decode |
| `anyview-raw` | LibRaw's `dcraw_emu`, else `dcraw` | `LibRaw-samples` (and `dcraw`), `libraw-bin` (and `dcraw`) | `dcraw_emu -w` on a symbolic link in a scratch folder (it writes beside its input), reading the 8-bit PPM it writes; `dcraw -c -w` writes to standard output. The thumbnail is the embedded preview (`-e`), else a scaled development |

Acceptance of both against real tools is still to do: the tests run stand-in scripts for the tools.

**How the viewer routes a decode.** `anyview_ui::ImagePlugins` is the seam (`Edge::with_image_plugins`, carried
on `OpenLink`); the binary's `host::ImageHost` implements it over the registry: `Plugins::route(Decode, subject)`
is `Served` (it asks the plugin with `PluginRunner::decode`, on the worker that opens the file, never the UI
thread), `Missing` (a `Needs` row naming the package) or `Unserved`. A plugin that is installed whose tools are
not says so in `hello`, and the row names the tools. The raster back end asks it for HEIC (and AVIF in a build
without its own decoder): pixels become the picture, `Missing` becomes a card of the file's facts and the `Needs`
row (`RasterDoc::needs`, shown like a recording without a player), and `Unserved` stays the old
`Unsupported` error.

**A raw file** is shown without any plugin from the JPEG preview inside it: `anyview_image` finds every baseline
or progressive JPEG stream in the file by its markers (a tag walk would differ by maker), checks each to its end,
and takes the one with the most pixels, so the sensor data (lossless JPEG) is never taken for a preview. That
covers the TIFF-based raws (CR2, NEF, ARW, DNG, ORF, RW2, PEF, SRW…) and Canon's CR3 (an ISO media file with the
preview in a box); a raw whose only preview is not JPEG shows no picture and needs the plugin. Orientation is the
preview's own EXIF orientation, else the one in the raw file's first IFD. The stage shows that preview as its
first frame at once; when the RAW plugin is installed and works its full development replaces it, and when it
is missing the preview stays, with a `Needs: anyview-raw (to show it in full quality)` row.

### Missing tools

A plugin whose tool is missing says so (the `Needs` row), and the viewer offers to install the tool the way Totem
offers a codec, through quire's shared missing-helper parts and never through anything of its own:

- **The data file** `dist/helpers/anyview.toml`, installed to `<prefix>/share/quire/helpers/anyview.toml` (covered by the
  receipt and by uninstall), has one table for each of `anyview_core::Helper`'s four tools (`video-playback`,
  `media-probe`, `heic-decode`, `raw-decode`: the slugs of the enum, which a test holds equal): the name and purpose the
  sheet says, the executables that prove the tool is there, and the package names for `dnf`, `apt`, `pacman` and
  `zypper`. Each name is marked verified (looked up on Fedora 44) or not in a comment.
- **`host::HelperHost`** wraps `ds_helpers::Helpers` over that file (found in the person's data directory, then
  `XDG_DATA_DIRS`) and PackageKit. `words` words the sheet from the file; `need` turns a `Needs` row into a `Need` that
  carries the tool only when the file declares it; `provide` runs `Helpers::provide` as a host task
  (`Task::Provide`, on the platform runtime, never the UI thread) and maps its `Outcome` to `HelperEnd`
  (`Installed`, `Declined`, `NotFound`, `Unsupported`, `Failed`); the package manager's words go to the log through
  `feedback::line_of` and to the sheet, never a toast. A host with no file offers nothing, and a missing tool stays a row.
- **Where the offer is made.** The row stays as the passive state. `ImageHost` gives a HEIC or raw file whose plugin
  is installed and whose tool is not (`PlatformError::PluginLacks`) an Install… button; `PlayerHost` gives a recording
  with no player one when the mpv plugin is installed and its mpv is absent (`Plugins::tool_absent`: the manifest's
  `mpv` is a missing file), and none when the plugin itself is absent or its C plugin is, since a tool would not help. A
  raw file shown from its embedded preview has no button (a picture is on screen; nothing asks), but it reopens with
  the full development when the tool appears. The question is asked at the moment of use: the person presses Install…,
  and nothing opens a sheet by itself, not even on Play (a recording that cannot play is a card, as it always was).
- **The sheet** is quire's `HelperSheet`, driven by the `Sheet::Helper` machine state: Return installs, Escape is Not
  Now, nothing does anything while the system installs (the password prompt is the system's). `Installed` closes the
  sheet and reopens the file in place (`ViewerIn::Reload`); `Declined` closes it quietly; the other ends are the
  sheet's own phases, closed by Return or Escape.
- **Hearing of a tool from elsewhere.** `HelperHost::following` (spawned on the platform runtime) awaits
  `Helpers::subscribe`; `PathWatch` watches the folders of `PATH` and, after a burst settles, `look_again` calls
  `Helpers::refresh` for each tool. A tool that appears makes `PluginRegistry::refresh` read the manifests again (the
  plugins' hellos are asked afresh on every request already) and then tells every window (`Edge::available`): a window
  whose file lacked that tool reopens it, a sheet that was asking closes, and held neighbours that lacked it are let go.
- **Tests** use `Installer::Fake` only: the sheet's phases (`sheet/tests.rs`), a window under the harness
  (`anyview-ui/tests/ui/missing_helpers.rs`), the host (`host/tests/helpers.rs`) and the real HEIF plugin end to end
  (`anyview/tests/anyview/missing_helpers.rs`).

## 2m. Processes, and what to install

The viewer links no codec and no copyleft code: video and audio are played and probed by programs the person
installs, which it runs (CONVENTIONS section 15, "run, never link"). The processes of one running viewer:

```text
anyview                          the viewer: windows, the media thread, the pool; MIT OR Apache-2.0
|                                reads $XDG_DATA_DIRS/anyview/plugins/*.toml once (`discover`)
|
+-- mpv                          the person's own, one for each playing recording (a child process)
|     |                          mpv --no-config --idle=yes --vo=libmpv --script=<cplugin> ...
|     +-- mpv-wgpu-cplugin.so    ours, loaded by mpv with --script; contains no mpv code
|           frames    a three-slot memfd ring mpv draws into, mapped by the viewer and uploaded
|                     into the window's wgpu texture
|           commands  a Unix socket: load, seek, properties, events, screenshots
|
+-- anyview-heif, anyview-raw    the picture plugins: one process for each decode or thumbnail; they run
|     |                          the person's heif-dec / dcraw_emu on a scratch folder and read the PNG or PPM
+-- anyview-ffmpeg               the FFmpeg plugin: one process for each request (facts, thumbnail, export),
      |                          killed when the call returns or is dropped; protocol v1 over its pipes
      +-- ffprobe, ffmpeg        the person's own, run by the plugin; progress on a pipe
```

A crash is an event, never a viewer crash: a dead mpv is `PlayerGone` and the stage shows that the player
stopped; a plugin that dies, hangs or lies costs one request.

**Runtime packages.** The viewer runs without either, and says so: a recording nothing plays opens as
its facts with a `Needs: anyview-mpv` row, and the export sheet offers no recording formats and says which
package adds them. To play and convert, install the distribution's `mpv` and `ffmpeg` (the patent-encumbered
codecs come from there: Fedora's `ffmpeg-free` plus RPM Fusion's `ffmpeg`, Debian's `ffmpeg`), and the two
plugins, which a plain `dist/install.sh` installs: it builds mpv-wgpu's C plugin (from `--with-mpv-from`,
`MPV_WGPU_DIR` or `../mpv`, else a shallow fetch of the revision pinned in `install.sh` into
`$XDG_CACHE_HOME/anyview/build`) and the FFmpeg plugin, finds `mpv` on the search path at install time
(`--mpv PATH` names another) and writes `mpv.toml` and `ffmpeg.toml` under `<prefix>/share/anyview/plugins`.
Distribution packages are named `anyview-mpv` and `anyview-ffmpeg`.

Pictures the viewer cannot decode work the same way: a HEIC opens as its facts with a `Needs: anyview-heif` row
until the distribution's libheif tools are there (the plugin itself installs with the viewer), and a raw file shows
its embedded preview with a `Needs: anyview-raw` row until LibRaw's `dcraw_emu` (or `dcraw`) is installed. Distribution packages: `anyview-heif` and `anyview-raw`. Each of these rows offers Install… (see "Missing tools").

## 2o. Modules inside `anyview-book`

Same rules as section 2. Blocking and effect-free except the reads of the one file it is asked about;
the zip is read through `anyview-archive`, one entry at a time.

| Module | Holds |
| --- | --- |
| `error` | `BookError` |
| `epub` | `Epub` (open: container, package document and spine; `contents`, `chapter`), `EpubMeta`, `TocEntry`; `package` (metadata, manifest, spine, cover), `contents` (the EPUB 3 navigation document, else the EPUB 2 NCX, else the chapters by number) and `chapter` (the zip's files behind a chapter, with what it may take in) are private |
| `comic` | `Comic`, `ComicPage`: the images of a zip in natural order, one page's bytes |
| `natural` | `natural_order`: digits by value, text without case |
| `seal` | `Chapter` (`styles` and `body`), `seal`: a chapter's markup rebuilt from an allowlist (`element`), its styles sealed (`css`), its images inlined through `Assets`; `tokens` (a forgiving HTML tokenizer), `walk`, `entities` are private |
| `cover` | `Cover`, `epub_cover`, `comic_cover`: one image's bytes, never decoded here |
| `zip_path` | the references a package makes, resolved to entry names (private) |

A chapter is sealed by writing only what is known: elements and attributes come from allowlists, so a
script, a frame, a form, an event handler or an unknown element never reaches the page (its text does).
Stylesheets lose `@import`, `@font-face` and every `url()` that is not a file of the package, which is
inlined as a `data:` URL, and so do images (an SVG `image` becomes a picture; an image the package does
not hold becomes its alt text). A link keeps only a fragment or a web or mail address. A chapter takes in
at most 24 MiB of files. The viewer lays each chapter out on a fixed reading page and binds the chapters as one PDF the PDF
stage shows (`families/pdf/bound`, below); fixed-layout books, scripts, audio and video, fonts and DRM are not supported.

### Books as PDFs

An EPUB and a comic zip are shown by the PDF stage (the registry maps `FormatKind::Book` to
`PdfStageView`), so they have its page stack, find, zoom, thumbnails, contents and resume (`Resume::Pdf`:
page and offset) and one wheel, with no frame to swallow it. `families/pdf/bound` makes the document on the
open worker: `epub` lays each chapter (sealed HTML with `data:` images, plus the book's styles) out with
`ds_blitz::pdf` on one fixed page of 776 by 1164 points, text 680 wide in 14 point Inter at line-height 1.6
(`book.css`: the Reading face of the text family, in points so that a page at 100% reads at `--fs-reading`);
`comic` puts each picture on a page of its own size (`anyview_pdf::pdf_of_pictures`, twelve at a time so a
comic is never all decoded at once); `anyview_pdf::bind` joins the parts and writes the outline: each line of
the book's contents goes to the first page of its chapter, and, where several lines lead into one chapter file,
to the first page from the previous line's on that shows the line's words. The result is kept for the session by
file and stamp (`kept`: four books, 256 MiB) so opening it again is instant; nothing is written to disk. The
`StageFamily`, `Stage` and `Job` of a book no longer exist; `Resume::Book` stays in `anyview-core` only so an
old store still reads.

## 2n. Modules inside `anyview-export`

Same rules as section 2: private modules, each public item re-exported once at the crate root. Blocking
and on the caller's worker: no runtime, no spawning, no clock. Each format plans its own jobs next to its
decoder (`anyview_image::plan_export`, `anyview_pdf::plan_export`, `anyview_text::plan_export`); this crate
runs `ExportJob`s and knows no format's options.

| Module | Holds |
| --- | --- |
| `error` | `ExportError` (`Image`, `Pdf`, `Text`, `Layout`, `Path`, `Read`, `Write`, `Exists`, `NoFreeName`, `NothingToWrite`, `NotADocument`) |
| `choice` | `DocumentExport` (`Raster`, `Pdf`, `Text`): what a person chose of a file that is not a recording |
| `run` | `export` (plan, run each job, write each file; the files already written are removed if a later one fails) and `printout` (a PDF as it is, an image or a text document laid out as one) |
| `name` | the suffix each job adds (`page 3`, `pages 2-4`, `copy`) and where a job's file goes (`anyview_store::free_beside`: the first free name) |
| `write` | `write_new`: a hidden temporary file beside the destination, made new, synced, linked to the destination; an existing destination is never replaced, and the hidden files of dead processes are swept first |
| `produce` | the one match on `ExportJob`: a job as the bytes of one file; `raster`, `pdf`, `pictures` and `print` are its parts, and `session` holds what the jobs of a run share (the open PDF and its worker, the highlighter) |

An export is a copy beside the original under a free name, never the original: a JPEG exported as a JPEG
is `photo 2.jpg`. A JPEG that is already upright goes into a PDF as the file it is, any other picture as
pixels, and an SVG as vector paths. A printout is made in memory and handed to the printer; it leaves no
file. A recording's export (`Transcode`, `MpvScreenshot`) is not a document's and is `NotADocument` here.

## 3. Layer rules

1. **A lower layer never names a higher one.** If something needed lives above, move the shared
   piece down or pass it in.
2. **`anyview-core` is pure.** Effects (reading files, listing a zip, decoding, playing) belong to
   crates above it, which hand it values: the first 4 KiB of a file, a zip's entry names.
3. **Every kind of file is mapped from day one.** A kind without a viewer stage is `PeekOnly` (facts
   only), never a stub.
4. **No `_` arm on our own enums.** The workspace turns on clippy's `wildcard_enum_match_arm` and
   `match_wildcard_for_single_variants` at `deny`. An input a state ignores is listed by name, so
   adding a variant is a compile error everywhere it matters.
5. **One exhaustive match per enum, in one file.** `FormatKind` is matched in `profile`;
   `FileAction` in `action`'s spec; each format family in its own file behind the `Family` trait.
   A second match elsewhere is a missing method.
6. **Typestate where a lifecycle is linear and owned in one place.** `Sniffed` can only be made by
   sniffing, so a loader that takes one is never handed a kind guessed from a name alone.

## 4. One home per concept

The single place a concept lives. Extend it; never write a second one.

| Concept | Home |
| --- | --- |
| Closed vocabulary (slug, label, ALL, parse) | `ds_core::word::Word` and `#[derive(Word)]` |
| Shortcuts and the reserved standard table | `ds_core::vocab::Shortcut`, `ds_core::standard_action::StandardAction` |
| What a file is (kind, MIME, detail) | `anyview_core::FormatKind`, `Mime`, `FormatDetail` |
| Sniffing, magic bytes first, extension as the fallback | `anyview_core::sniff`, `sniff_zip` |
| Which extensions and MIME types a format family has | `Family` in `kind/family.rs`, one file per family |
| Which actions, edits and stage a kind has | `anyview_core::actions_for`, `edits_for`, `stage_support` (`profile`) |
| Where an action may appear, and its keys | `anyview_core::reach`, `shortcut` (`action/spec.rs`) |
| Page, time, volume, zoom, turn, dpi, pixel units | `anyview_core` units (`units`) |
| The list the arrow keys walk | `anyview_core::Sequence`, `moved`, `neighbours` |
| Where a person left a file | `anyview_core::Resume` |
| Where it is kept between runs, and the recently-viewed list | `anyview_store::StoreWriter`, `read_history` |
| The folder the store lives in under the data directory (the launcher reads it too) | `anyview_store::STORE_FOLDER` |
| A kind's commonest media type, and the kind a media type names | `anyview_core::mime_for` (`profile`), `kind_of_mime` (`kind/of_mime.rs`) |
| A file handed to the viewer with its results and place, and its D-Bus form | `anyview_platform::Handoff`, `Request::Handoff`, `handoff.rs`; the viewer's half is `anyview::Opening::handed` and `HandedResume` |
| The "Page 143" a history row shows | `anyview_store::resume_label` |
| What each format exports, and the sheet's contract | `anyview_core::ExportChoice` and the per-format enums (`export`) |
| The shared encoders an export becomes | `anyview_core::ExportJob` |
| Rows of facts a pane lists | `anyview_core::Facts`; a row's section is its `FactGroup` and its weight its `Tier` |
| What every file has (kind, size, dates, where, where from, permissions) | `anyview_core::Facts::general`, read by `anyview_store::file_details`; the window adds it last to every document's rows |
| Where a photo was taken | `anyview_image::Location::of_file` and `Location::facts`, in the viewer's Info panel only (`ExifFacts`, and so the peek, never holds it); no preview, list or thumbnail lists it, and serial numbers are never read |
| The light tier of a format | `anyview_core::Peek` |
| Which peek a kind has (the light tier's one match on `FormatKind`) | `anyview_peek::visit`, `KindVisitor` (`registry.rs`) |
| A peek's result with its type erased, and the rows beside it | `anyview_peek::AnyPeeked`, `peek`, `Body` |
| What a kind with no back end yet shows | `anyview_peek::FactsPeek` (`described.rs`) |
| A folder's count, size and kinds | `anyview_peek::FolderPeek` |
| The first page of a PDF for a pane | `anyview_peek::PdfPeek`, over `ds_blitz::pdf_thumb_blocking` |
| An archive's entries, and one entry out | `anyview_archive::list`, `extract` |
| A zip's entry names for `sniff_zip` | `anyview_archive::zip_entries` |
| An EPUB's package, a chapter as sealed HTML, a comic's pages in order | `anyview_book::Epub`, `Comic`, `natural_order` |
| What a path is: stat, head, zip entries, sniffed | `anyview_peek::probe` (the viewer's `io::probe` calls it) |
| A file looked at: probed, then peeked as `Peeking` says | `anyview_peek::look`, `Peeking` |
| A path as the `Input` a reader takes | `anyview_fs::OnDisk` |
| Why a card has no peek | `anyview_peek::Unavailable` |
| Which container an archive format is stored in | `container.rs` in `anyview-archive` |
| A font's names, glyph count and specimen outlines | `anyview_font::FontPeek`, `Face`, `Specimen` |
| A modification time as words | `anyview_peek::modified_text` |
| Drawing what was peeked at | `anyview_peek::Pane` |
| A picture on the screen without a PNG `data:` URL | `ds_blitz::TextureLayer`, from `anyview_peek`'s `pane/picture.rs` |
| Token classes to colours | `anyview_peek::STYLE` (`pane/pane.css`), the `tok-<class>` rules |
| The crate error | `anyview_core::CoreError` (`ImageError` in `anyview-image`, `TextError` in `anyview-text`) |
| Raster and vector pixels out of a file (RGBA8, straight alpha, upright) | `anyview_image::decode`, `decode_bytes` |
| Straight to premultiplied alpha | `anyview_image::Rgba8::premultiplied` |
| Which decoder a raster format uses | `decode/codec.rs` in `anyview-image` |
| EXIF orientation as a mirror and a turn, and applying it | `anyview_image::ExifOrientation` |
| Camera, lens, exposure and date of a photo | `anyview_image::ExifFacts` |
| Fitting a picture to a peek budget, and the export resize | `scale.rs` in `anyview-image` (`resized`) |
| The image peek and its facts | `anyview_image::RasterPeek`, `VectorPeek` |
| Raster export encodes (PNG, JPEG, WebP, AVIF, TIFF, BMP) | `anyview_image::encode`, `encode_bmp` |
| Keeping EXIF and ICC across a re-encode | `anyview_image::encode_with_metadata` |
| Rotating a JPEG without re-encoding | `anyview_image::rotate_jpeg` |
| The encoding of a text file, and decoding it | `anyview_text::detect`, `TextCodec` |
| Reading a window of lines from a file of any size | `anyview_text::TextLines` |
| Highlighting a window of lines into token classes | `anyview_text::CodeLines`, `Highlighter` |
| Which class a syntax scope is | `code/class.rs` in `anyview-text` |
| Highlighted code as HTML | `anyview_text::tokens_html` |
| Markdown to HTML, and its outline | `anyview_text::render` |
| Which link targets and images a rendered document keeps | `markdown/links.rs`, `markdown/images.rs` in `anyview-text` |
| Escaping text for HTML | `escape.rs` in `anyview-text` |
| CSV and TSV rows, and the header guess | `anyview_text::Table`, `HeaderMode` |
| JSON and JSON Lines, one level at a time | `anyview_text::Tree`, `TreePath` |
| The text, code, Markdown, table and tree peeks | `anyview_text::PlainPeek`, `CodePeek`, `MarkdownPeek`, `TablePeek`, `TreePeek` |
| Which tiles a view needs, in what order, at what zoom | `anyview_pdf::schedule`, `ZoomBucket` (`tile/schedule.rs`) |
| A tile's pixels, and the page as laid out | `anyview_pdf::PdfBackend::run` (`PdfJob::Tiles`), `PageLayout` |
| Page space to displayed space (crop box, `/Rotate`) | `anyview_pdf::displayed` (`geometry.rs`) |
| Searching a PDF, and the hit a find shows | `anyview_pdf::search_document`, `Hits` |
| A PDF's outline and the links on a page | `anyview_pdf::outline`, `page_links` |
| Rotating, deleting and moving PDF pages | `anyview_pdf::apply`, `PageOp` |
| What a PDF export is made of | `anyview_pdf::plan_export` |
| What an image export and a text export are made of | `anyview_image::plan_export`, `anyview_text::plan_export` (`plan_print`: a text printout) |
| The jobs of an export as files, and the PDF a printer takes | `anyview_export::export`, `printout` |
| The one match on `ExportJob` for documents | `anyview_export`'s `produce.rs` |
| The name an export is written under (a free name beside the original) | `anyview_export::free_beside` (a media export's frame uses it too) |
| Writing a file whole, never partial, never over another | `anyview_export`'s `write.rs` (`write_new`) |
| Images on the pages of a PDF | `anyview_pdf::pdf_of_pictures`, `PagePicture` |
| A text document as the page that is printed | `anyview_text::printable_html` |
| The files a Markdown document refers to, read from the disk | `anyview_text::DiskFiles` |
| The contract a back end is run through, and cancelling it | `anyview_core::work` (`Backend`, `Stop`, `Ticket`; `anyview_pdf` re-exports them) |
| A pure timed state machine and its time | `ds_core::machine::Machine`, `ds_core::time::stamp::Stamp` |
| When the hover chrome shows and hides, and what holds it up | `anyview_ui::Chrome`, `PinReasons` |
| Which region a key goes to | `anyview_ui::route` (`keys/route.rs`) |
| What a command or a key means to the showing stage | `Stage::input_for` (`stage/dispatch.rs`) |
| Which keys stand for a stage command | `StageCommand::from_key` (`command.rs`) |
| Ignoring a result that arrived after the person left a file | `anyview_core::work::Ticket` (re-exported by `anyview_ui`), `Load`; `Ticketed` pairs a result with it |
| What a back end offers a worker, and how work is told to stop | `anyview_core::work::Backend`, `Stop` |
| The threads: the worker pool, its lanes, panics in jobs | `anyview::runtime::Pool`, `Runner` |
| Handing a result to the UI thread and waking it | `anyview::runtime::Mailbox`, `Outbox`, `UiWaker` |
| An object one thread owns, with commands in and events out (the player) | `anyview::runtime::Actor`, `ActorBody` |
| The views' work on the pool | `anyview::seam::Workforce` (the one `Workers`) |
| The command line | `anyview::cli::parse` |
| Being the viewer, or forwarding a launch to it | `anyview::program::claim_role` over `anyview_platform::Instance` (`DbusInstance` or `portable::LatchkeyInstance`) |
| What a window's request means to the host | `anyview::host::route` |
| Telling a window that its file changed on disk | `anyview::host::Watcher` (`WindowWatch`), calling `Edge::changed` |
| Writing where the person is, not for every scroll | `anyview::host::Remembering` |
| Carrying out reveal, share, print, trash, rename, duplicate, the history | `anyview::host::Desktop` (`Hosting::carry_out`) |
| Moving a file to the trash | `anyview::host::Trash`, `SystemTrash` |
| The one writer of the history and view memory, and the time it stamps | `anyview::host::Store`, `Clock` |
| A window of the viewer | `anyview::window::open_in_window`, `Seed` |
| Stepping to the next or previous find hit, wrapping | `anyview_ui::FindHits` (`stage/find.rs`) |
| A phrase found in a text, as hits that cut a line at character boundaries | `anyview_text::Needle`, `FindHit`, `TextLines::find` |
| The palette as a find: the scope, the hit rows and "Show All", and the stage's find following it | `PaletteScope`, `HitList` (`palette/model.rs`), `rows_for` (`views/session.rs`), `find_synced` (`viewer/step.rs`), `views/palette.rs` |
| The capsule's readout and steps while a find has hits | `families/found.rs` (`standing`) |
| How soon a job is wanted, and which pool lane that is | `Job::lane` -> `WorkLane`; the binary's one mapping to the runtime's `Lane` (`seam/workforce.rs`) |
| Which hits are on a line, how a line is cut at them, where the view scrolls to show one | `FoundHits`, `pieces`, `top_for` (`families/text/find.rs`) |
| How many lines fit a page when long lines wrap | `families/text/wrap.rs` |
| Where a key step through a text lands | `stage/text/steps.rs` |
| What a stage remembers of where the person is, and puts back | `Stage::resume`, `Stage::restoring` (`stage/resume.rs`) |
| Which files refuse a save in place (their edits are not offered) | `anyview_ui::FileLocks` and `Probed::access` (the binary implements it over `anyview_store::is_read_only`: `host/locks.rs`) |
| Which rows the right-click menu has, in what order | `context/entries.rs` (`entries`), over the palette's `commands` (`views/session.rs`) |
| Where a file was left, read by the window | `anyview_ui::ResumeSource` (the binary implements it over `anyview_store`) |
| Where a file is left, kept | `HostRequest::Remember` (the binary writes it through `anyview_store`) |
| The cheap first frame of a file | `StageView::first_frame` (`families/view.rs`); a picture's from `anyview_peek::StillSource` (the host's thumbnail cache), an animation's or a vector's from `anyview_image`'s peeks, a text's the first bytes of the file (`families/text/doc.rs`) |
| The size a picture will have, without decoding it | `anyview_image::declared_size` |
| The files opened ahead, and the one just left | `views/preloads.rs` |
| The folder of a file as the list the arrow keys walk, in name order | `anyview_ui::folder_sequence` (`io/folder.rs`) |
| Whether a changed file is reloaded | `anyview_ui::freshness` (`load/fresh.rs`); the host says a file changed through `Edge::changed` |
| The frames of an animation, and the clock that plays them | `RasterDoc` strip (`families/raster/doc.rs`) for the textures and delays; the clock is `RasterStage::wake` (`stage/raster/step.rs`) |
| The zoom a step in or out lands on, and the point it holds still | `stage/zoom.rs` (`stepped`, `centre_about`) |
| The person's directories, the session bus and starting a program | `anyview_platform::Env` (`env.rs`); nothing else reads `std::env`, `dirs` or a bus address |
| One viewer process, and forwarding a launch to it | `anyview_platform::Instance`, `Request` |
| Now playing and the desktop's media controls | `anyview_platform::MediaSession`, `MediaState`, `MediaControl` |
| The shared thumbnail cache, and a file's `file://` URI | `anyview_platform::ThumbnailCache`, `file_uri` |
| What a plugin says of itself (the manifest), and its checks | bayonet's `manifest` (the envelope), `anyview_plugin::Manifest`, `PluginError`, `Provision` (the viewer's `[[provides]]` entries) |
| Which plugin serves a kind and capability, and which wins when manifests collide | bayonet's `registry` (the ranking), `anyview_plugin::Plugins` (`resolve`, `serving`, `route`, `export_targets`) |
| The package to suggest for a kind no plugin serves, and the facts row that says it | bayonet's `Package` and `suggest`, `anyview_plugin::suggested_package` (`missing.rs`), `MissingPlugin::fact` |
| The plugin protocol's messages | `anyview_plugin_protocol` (`HostMessage`, `PluginMessage`) |
| Framing: the length-prefixed JSON a message travels in | bayonet's `wire` (`encode_frame`, `read_frame`, `write_frame`, `FrameDecoder`), re-exported by `anyview_plugin_protocol` |
| Finding manifests on disk and checking their programs | bayonet's `discover`, called by `anyview_platform::discover` |
| Starting a plugin, its timeouts, cancel and kill | bayonet's `run` (`Runner`, `Session`) |
| Asking a plugin something | `anyview_platform::PluginRunner` |
| The facts of a kind with no built-in back end, from a plugin or the package that is missing | `anyview_platform::PluginRunner::peek_facts` |
| Printing, sharing, revealing a file, keeping a window above | `anyview_platform::Printer`, `Share`, `Reveal`, `WindowStacking` |
| Which view shows a kind of file, and opening it | `anyview_ui::visit`, `family_of` (`families/registry.rs`) |
| What a family draws, controls and lists | `anyview_ui::StageView` (`families/view.rs`) |
| Where a picture lands in the room, and which texels show | `families/raster/geometry.rs` |
| Where the pages of a PDF are in the room, what a zoom means for it, and the tiles the room asks for | `families/pdf/scene.rs` (`Scene`, `fits`, `scale_of`) |
| Which PDF tiles are on the GPU, in flight or given up on, and the hits, thumbnails and links of the open PDF | `families/pdf/live.rs` (`PdfLive`, held by the window as `PdfShelf`) |
| Which PDF tiles a cache lets go of | `families/pdf/cache.rs` (`evictions`) |
| Drawing, uploading and searching for a PDF stage on a worker | `families/pdf/work.rs` (`PdfTask`, `PdfAnswer`) |
| How the PDF room moves (scroll, drag, pinch, a jump, a hit) | `families/pdf/steer.rs` |
| A key event as a `Shortcut` (⌘ is Control or Command) | `views/keys.rs` |
| Running a blocking job off the UI thread | `anyview_ui::Workers`, `Work`, `Job::run` (`io/`) |
| What a window asks of the binary | `anyview_ui::HostRequest` (`io/workers.rs`) |
| The viewer's own stylesheet and the token colours of code | `anyview_ui::stylesheet`, `TOKEN_CSS` |
| What lets the root's regions affect each other | `Viewer`'s `step` (`viewer/step.rs`), `viewer/pins.rs`, `viewer/command.rs` |
| A player with its states in the types (tracks, seek and volume exist only once a file is loaded) | `anyview_media::Session` (`Idle`, `Opening`, `Loaded`) |
| One player on the media thread: instructions queued until the file opens, the position, the end of a held-open file, the texture announced | `anyview_media::Driver` (mpv), `FrameSink`; behind the one trait `MediaDriver`, which `BuiltinDriver` (audio, no mpv) also implements |
| Which player a recording gets (mpv, the built-in audio player, or the row that names what would play it) | `media/engine.rs` (`choose`, a pure function of the plugin route and `BuiltinAbility`) |
| The sound card, and a fake one a test drains by hand | `anyview_media::SoundOutput` (`CardOutput`, `SilentOutput`) |
| The player's instructions and news, as data | `anyview_media::MediaCommand`, `MediaEvent` |
| What a player says of a recording (tracks, chapters, tags, whether a picture shows) | `anyview_core::MediaTrack`, `MediaChapter`, `MediaTags`, `VideoPresence` (`media`) |
| Speed, chapter, trim range, bitrate | `anyview_core::Speed`, `ChapterIndex`, `TimeRange`, `Bitrate` (`units`) |
| A recording's facts and tags | `media::MediaPlugins::reading` in the binary: the FFmpeg plugin's rows, else `anyview-peek`'s |
| A recording in the launcher's pane | `anyview_peek::VideoPeek`, `AudioPeek` (pure-Rust parsers, `media/`), drawn by `anyview-peek`'s pane |
| A video's frame in the launcher's pane | `anyview_peek::StillSource`, `Peeking::with_stills` (the host's thumbnail cache; `frames.rs`) |
| What a media export becomes, what it asks of the plugin, and its name | `anyview_media::plan_export`, `ask_of`, `output_path` |
| Which media exports a recording offers | `media::MediaPlugins::offer` (the plugin's greeting and the file's kind), `anyview_ui::MediaOffer` |
| Cutting, copying and converting a recording, with progress and stop | `media::PluginExport` (a pool job asking the FFmpeg plugin), `media::Exports` in the binary |
| The frame on screen, saved | `MediaCommand::Screenshot` on the player that shows it, then `host/media.rs` encodes it as the format asked |
| Which players run, the desktop's one entry and its controls | `anyview::media::MediaHub` |
| What a desktop control means to a player | `orders_for` (`media/orders.rs`) |
| The now-playing entry built from the player's events, and how often a moving position is published | `media/snapshot.rs` (`Snapshot`) |
| The player's events and instructions as the stage machine's, both ways | `media/map.rs` |
| Starting a player for a window | `anyview_ui::MediaHost`, implemented by `anyview::media::PlayerHost` |
| What a window holds of its player | `anyview_ui::MediaLine`; the news is applied by `MediaShelf` (`families/media/shelf.rs`) |
| Playing with no window | `MediaHub::play_in_background`: it holds the event loop open (`ds_blitz::AppHandle::hold`) while it plays |
| The small window of a recording | `Presentation::Mini`; `WindowTask::Reopen` makes the window again; `WindowStacking` asks the desktop to keep it above |
| The media capsule's controls | `families/media/capsule.rs`, over quire's `CapsuleSlot::Scrub` and `Level` and its `Scrubber` |
| Which capsule slots fit the stage's width | quire's capsule (`ds::components::chrome::capsule`): a family hands it `RankedSlot`s (`.essential()` or `.droppable(rank)`) and the capsule drops the highest rank still showing, a rank at a time, until the rest fits the stage it has measured; the media, PDF and picture capsules rank their slots, the rest are `essentials` |
| The tracks, speed and chapters panel | `families/media/panel.rs` |
| The trim marks an export is cut by | `TrimMarks` (`families/media/live.rs`), set by `MediaOut::Marked` |
| Where a recording is left | `Resume::Media`, put back by `MediaIn::Restore` (told at once, applied by the driver when the file opens) and kept by `views/arrive.rs` |

## 5. The canonical traits

A trait exists where two or more implementations swap or a generic consumer runs over many
(CONVENTIONS section 5). Closed sets stay enums.

```rust
/// The light tier of one format: cheap, no GPU, no player. One implementation per kind.
pub trait Peek: 'static {
    const KIND: FormatKind;
    type Peeked: Clone + PartialEq + Send + 'static;
    type Error: std::error::Error + Send;
    /// Blocking; run on a worker. Bounded by `budget` (bytes read, pixels decoded, time).
    fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Result<Self::Peeked, Self::Error>;
    fn facts(peeked: &Self::Peeked) -> Facts;
}

/// One format's closed set of exports. The export sheet is generic over it.
pub trait ExportChoice: Clone + PartialEq + 'static {
    type Kind: Word;
    fn kinds() -> &'static [Self::Kind] { Self::Kind::ALL }   // the format pop-up's entries
    fn kind(&self) -> Self::Kind;                            // the pop-up's current entry
    fn default_for(kind: Self::Kind) -> Self;                // options pre-filled
    fn extension(&self) -> ExportExtension;
}
```

`anyview-core` holds no registry and no visitor over `Peek`: it declares the trait and cannot name
its implementations. The light tier's registry (one exhaustive match over `FormatKind`) lives in
`anyview-peek` and the stage registry in `anyview-ui`:

```rust
pub trait Light: Peek<Peeked: Into<Body>, Error: Into<PeekError>> {}   // blanket: every peek here is one
pub trait KindVisitor { type Out; fn visit<P: Light>(self) -> Self::Out; }
pub fn visit<V: KindVisitor>(kind: FormatKind, visitor: V) -> V::Out;  // the one exhaustive match
```

Two traits belong to the back ends, each because a fake swaps for the real thing in tests:
`anyview_text::ByteSource` (read a range of bytes: `FileBytes` on disk, `HeldBytes` in memory) and
`anyview_text::LocalFiles` (read the files a Markdown document refers to: the edge's disk reader, and
a map in a test). `TextLines` and `CodeLines` are generic over the first so a window of lines is
tested without a file.

`Machine` is quire's pure state machine trait (`ds_core::machine`); every viewer region implements it
(section 5a). The viewer's `Stage` is the machine enum of the stage region; the full tier of a family
of formats is the trait `StageView`, named for what it draws:

```rust
pub trait StageView: 'static {
    const FAMILY: StageFamily;
    type Doc: Debug + Send + Sync + 'static;     // what opening makes: the picture on the GPU, the line index
    fn open(ticket: Ticket, src: &Source, sniffed: &Sniffed, link: &OpenLink) -> Result<Self::Doc, OpenError>; // on a worker
    fn facts(doc: &Self::Doc) -> Facts;
    fn tabs(doc: &Self::Doc) -> PanelTabs;
    fn params(doc: &Self::Doc, stage: &Stage, area: Option<Area>) -> StageParams;
    fn stage(doc: &Arc<Self::Doc>, cx: &StageCx) -> Element;                  // the content
    fn slots(doc: &Self::Doc, cx: &StageCx) -> Vec<CapsuleSlot<Command>>;      // the capsule's controls
    fn panel(doc: &Arc<Self::Doc>, tab: PanelTab, cx: &StageCx) -> Option<Element>;
    fn lines(doc: &Arc<Self::Doc>, ticket: Ticket, first: LineIndex, rows: u32) -> Option<Job>; // default None
}
```

The window holds a `LoadedDoc` (an `Arc<dyn DocView>` made by `LoadedDoc::of::<S>`), so drawing needs no
match on the family. `visit(kind, KindVisitor)` in `families/registry.rs` is the one exhaustive match over
`FormatKind` for the full tier: every kind maps to a view, and the kinds the full tier does not show yet
map to `PeekOnlyStageView` (facts, a real view). `anyview_core::stage_support` says the same in
the core's table, and a test holds the two equal. `Workers` (`io/workers.rs`) is the one seam to the
binary's pool: `fn submit(&self, work: Work)`.

## 5b. Threads

The binary owns every thread; libraries never spawn. Back-end crates expose `Backend::run` and
their work items and nothing else.

| Thread | Owner | Runs |
| --- | --- | --- |
| `anyview-watch` | `host::Watcher` | the file watcher's burst settling: it owns the one `notify` instance's events and calls `Edge::changed` from here |
| UI | the window (`ds-blitz`) | the machines, the views, `TextureLayer`, Markdown and HTML layout; never blocks |
| workers, `PoolSize::from_cores(cores)` (cores minus one, at least one) | `runtime::Pool` | back-end jobs, visible-lane first, and media exports (`media::Exports`) |
| `anyview-media` (one per recording that plays) | `runtime::Actor` | the media player (mpv's `Driver` or the built-in `BuiltinDriver`): built, polled and commanded only there; the built-in one decodes there, whenever the sound card's wake or an instruction brings a turn |
| (cpal's) | the sound card's callback | drains `anyview_media::Pipe` into the card and wakes the actor as it runs low or time passes; it decodes nothing and takes one short lock. `SilentOutput` (`ANYVIEW_AUDIO_OUTPUT=null`) is a thread of ours that drains the same pipe at the recording's pace |
| (none: plugin calls) | the worker that makes them | `PluginRunner` calls block their worker and poll the plugin's pipe, so they spawn no thread of their own; a plugin is a child process, killed when its call ends |
| async runtime | the binary's tokio runtime (`program::start`: one worker, `anyview-platform`) | `anyview-platform` (D-Bus, MPRIS) and the host's tasks (`host::Desktop`); blocking work among them runs on its blocking pool |

`ds-blitz` keeps a process-wide tokio runtime of its own (two workers, entered by `launch`) for the design
system's portal watch and timers; the binary's is a second one, started before the window so the instance can
be claimed first. A window's results reach its UI task through the `Edge`'s reply channel, whose waker is the
task's; the runtime's `Mailbox` carries only how each job ended, for the report of one that panicked
(`seam`).

The contract, in order of a job's life: a machine output becomes `Runner::submit(lane, ticket, doc,
job, deadline)`, which returns a `JobHandle` (the ticket and the job's `Stop`). A worker skips a
job whose `Stop` was raised or whose deadline passed before it started (`JobOutcome::Skipped`),
otherwise runs `B::run(&doc, &mut worker, job, &stop)` with the worker's own scratch (made once per
thread by the runner's `make_worker`, rebuilt after a panic). The result, `Ticketed<JobOutcome<Done>>`,
goes through the runner's `into_input` to the `Outbox`; the first post after a drain calls the
`UiWaker`, and the UI thread `drain`s the `Mailbox` and feeds each message to its machine, which
drops a stale `Ticket`. The runtime delivers every ticket and filters none. A panic in a job is a
`JobOutcome::Panicked` with the message, never a dead worker. `Stop` carries a flag and an optional
deadline; this crate reads no clock, so a back end asks `stopped_at(now)` with its own `Instant::now()`,
or reads `deadline()` for a limit of its own, or hands `flag()` to a back end whose own stop polls a flag. Dropping the `Pool` discards what is queued, lets running
jobs finish and joins the workers.

An actor owns an object that cannot be shared. `Actor::spawn` runs `make(ActorWake)` on the new
thread, so the object is built there and may be `!Send`; commands arrive through `Actor::send`, the
object's own callback calls `ActorWake::wake` (coalesced) and the body's `woken` polls it; events
go out through an `Outbox`. The player runs this way, one actor per recording that plays (a window's
or a background session's): `anyview::media`'s actor owns a `MediaDriver` (mpv's `Driver`, or the built-in `BuiltinDriver`
when there is no mpv and the file is audio), so `poll` is only ever
called on its thread (`crates/anyview/tests/media_thread.rs` checks that, FINDINGS). The window holds a
`MediaLine` to it: commands go in without blocking, the news comes back through the actor's mailbox, and the
window is woken by a `Done::Media` on its own reply channel (a wake that comes before the document has
landed is not lost: the line keeps what it has and the landing drains it). The picture is written into the
window's `TextureHandle` by the media thread itself, which then asks for a redraw; no frame passes through the
UI thread. Transcodes are pool jobs (`media::Exports`, a `Runner` over `ExportBackend` with a `JobHandle` to stop one).

## 5a. Machines

Each is a `ds_core::machine::Machine`: `step(self, In, Stamp, &Params) -> (Self, Vec<Out>)` and
`wake(&self) -> Option<Stamp>`. A state is an enum whose variants hold only that state's data; an
input a state ignores is listed by name; time arrives as a `Stamp`. A machine that waits on the
clock holds the instant it waits for in the state (`wake` takes no params), so a setting that
changes applies from the next step. Only the chrome keeps a timer; every other `wake` is `None`.

| Machine | States | Notable inputs | Outputs |
| --- | --- | --- | --- |
| `Chrome` | `Hidden`, `Revealing`, `Shown`, `Pinned { by: PinReasons }`, `Hiding` | `PointerMoved(Zone)`, `PointerLeft`, `Pin`, `Unpin`, `Elapsed` | `Fade { to, over }` |
| `Panel` | `Hidden`, `Shown { tab }` | `Toggle`, `Choose`, `Close`, `TabsChanged` | `Show(tab)`, `Hide` |
| `Palette` | `Closed`, `Open { query, selection }` | `Open`, `Typed`, `Move`, `Pick`, `Enter`, `Close` | `Opened`, `Closed`, `Run(Command)` |
| `ContextMenu` | `Closed`, `Open { at }` | `Open(Spot)`, `OpenAtCentre`, `Pick`, `Close` | `Run(ContextPick)` |
| `Sheet` | `Closed`, `Export { draft }`, `ConfirmTrash`, `Rename { name }` | `OpenExport`, `AskTrash`, `AskRename`, `PickKind`, `Change`, `Typed`, `Confirm`, `Cancel` | `Opened`, `Closed`, `Export`, `Trash`, `Rename` |
| `Navigate` | `Idle`, `Walking { sequence, heading }` | `Start`, `Next`, `Previous`, `First`, `Last`, `Gone`, `Leave` | `Open(path)`, `Preload(neighbours)` |
| `Presentation` | `Window`, `Peek`, `Mini`, `Background` | `ToWindow`, `ToMini` | `Become(presentation)` |
| `Load` | `Idle`, `Probing`, `Peeking { frame }`, `Opening`, `Ready`, `Failed { reason }`, each with its `Ticket` | `Begin`, `Probed`, `Peeked`, `PeekFailed`, `Opened`, `Failed` | `Probe`, `Peek`, `Open`, `Cancel`, `UseStage`, `ShowFirstFrame`, `ShowFull` |
| `RasterStage` | `Fitted`, `Zoomed`, `Panning`; an `Animation` (`Still`, `Playing { due }`, `Paused`, `Ended`) rides in each; `wake()` is the playing frame's `due` | `ZoomStep`, `SetZoom`, `DoubleClick`, `PanStart`/`PanBy`/`PanEnd`, `Rotate`, `Restore`, `Animated`, `TogglePlayback`, `StepFrame`, `Elapsed` | `Remember`, `Turned`, `ShowFrame` |
| `PdfStage` | `Reading`, `Finding { query, hits }`, `Jumping { target }` | `Scroll`, `SetZoom`, `Find`, `Results`, `NextHit`, `GoTo`, `NextPage`, `Arrived`, `Restore` | `Remember`, `ScrollTo`, `Find(..)` |
| `MediaStage` | `Opening`, `Playing`, `Paused`, `Scrubbing { resume }`, `Ended`, `Failed` | `Player(PlayerEvent)`, `Position`, `Toggle`, `Seek*`, `Scrub*`, `SetVolume`, `SetSpeed`, `StepSpeed`, `Select`, `CycleTrack`, `StepChapter`, `GoToChapter`, `Mark`, `Restore` | `Command(PlayerCommand)`, `Buffering`, `VolumeChanged`, `TracksChanged`, `Marked` |
| `TableStage` | `Browsing { sheet }`, `Selected { sheet, row }` | `NextSheet`, `PreviousSheet`, `ChooseSheet`, `Select`, `Deselect` | none |
| `TreeStage` | `Browsing { open }`, `Selected { open, row }` | `Toggle`, `Open`, `Close`, `CollapseAll`, `Select`, `Deselect` | none |
| `TextStage` | `Reading`, `Finding { query, hits }` | `Scroll`, `Step` (a line, a page, the start, the end), `Find`, `Results`, `NextHit`, `ToggleSource`, `ToggleWrap`, `Restore` | `Remember`, `ScrollTo`, `Show(view)`, `Find(..)` |
| `Stage` | `NoStage`, `Raster`, `Pdf`, `Media`, `Text`, `Table`, `Tree` | one family's input each | each family's output, lifted |
| `Viewer` | one state per region above | `Open`, `Reload` (a changed file: the stage stays), `Dropped` (the first file opens, and the walk ends; one's folder or the several are the list. `begin`, which every different file goes through, is what cancels a sheet, closes the palette and the context menu, and gives the stage and a held Space back; the pointer tool and the side panel stay), `Chosen` (the same, once the file chooser ends, with no files when it was cancelled; one chooser is asked for at a time), `StartAs` (the window was opened in a presentation: nothing is asked of the host), a region's input, `Run` (a command from a control the window drew), `Key` | each region's output, lifted; `Probe`, `Reload`, `ListFolder`, `Run`, `PickFile`, `CloseWindow` |

**Why the machines and the views share a crate.** The machines are the part that must stay pure, and
they are: each `model.rs` and `step.rs` names only `anyview-core` and `ds-core`, which the script
checks per file, and a machine's tests run with no window. A second crate for the views would add a
layer for no second consumer (the binary is the only one) and would split `Viewer`'s outputs from the
code that carries them out. The module list above is the boundary: the modules before `io` are the
machines, `io`, `families` and `views` are the edge. `anyview-ui` therefore names Blitz, vello and
anyrender through `ds-blitz`, which is why the root `Cargo.toml` carries quire's `[patch]` sections.

**Threads.** The library never spawns one. A window hands each blocking job to `Workers` (the binary's
one bounded pool) as a `Work`; a worker calls `Work::run`, and the `Done` it makes goes through the
`Reply` into the window's mailbox, where a task turns it into a machine input carrying the load's
`Ticket`. A decoded picture is uploaded into the window's `TextureHandle` by the worker itself, so
pixels never pass through the UI thread; a window of lines is read and highlighted on a worker and the
UI holds only the lines it returned. `Backend` and `Stop` are `anyview_core::work`, re-exported from `io`, the contract of the shared work
module.

**Behaviours over time.** The window keeps what workers made on its `Shelf` (signals), and each of these is a
machine input or a worker job, never a decision of a view.

- *First frame.* A family whose `StageView::FLOW` is `PeekThenOpen` gets `Job::Peek` beside `Job::Open`; a
  result that comes back is shown only while the full open has not (`Shelf::shown`), and the load machine's
  `Peeking` state counts it. A picture's first frame is the host's thumbnail when `StillSource` has one
  (shown in the box `declared_size` says the picture will fill), otherwise the first frame of a GIF or WebP or a
  drawn-small SVG; a large text's is its first 256 KiB.
- *Neighbours.* Each `Navigate` move says `Preload(neighbours)`; the window opens those two files on the
  `WorkLane::Preload` lane into textures of their own (`Job::Preload`) and keeps the file it just left as it was
  left. Arriving at a held file shows it with no probe and no open, and reads its stamp (`Job::Stat`) to see that
  it is still current.
- *Resume.* `Job::Probe` asks the host's `ResumeSource` for where the file was left (`Probed::resume`); after the
  load machine installs the stage, `Stage::restoring` turns it into the stage's own `Restore` input. Every
  settled gesture says `HostRequest::Remember`, which is how the host keeps it.
- *Access.* `Job::Probe` also asks the host's `FileLocks` whether the file takes a save in place
  (`Probed::access`); a file that refuses one has its picture edits, Revert To and page edits left out of
  the palette's commands, so the palette, the context menu and the keys all stop offering them.
- *Reload.* The host says a file changed (`Edge::changed`); the window reads its stamp and, if `freshness` says it
  differs from the one opened, sends `ViewerIn::Reload`. The root probes again with the stage left in place, a
  probe of the same family keeps it, and what is on screen stays until the new copy lands. The window asks the
  host to watch the shown file (`HostRequest::Watch`, replaced by the next); the watcher itself is the binary's,
  so no crate here names `notify`.
- *Drop.* The window is a drop target (`ds::file_drop`); `ViewerIn::Dropped` opens the first file, ends the walk,
  and asks for the folder as the new list (`ViewerOut::ListFolder`, answered through `folder_sequence`).
- *Animation.* An open that finds frames uploads each into a texture of its own; the stage machine's `Animation`
  says which is on screen and when it is due: `wake()` names the due time and the root's `Elapsed` advances
  the frame, reading the frame delays, the runs the file asks for and the desktop's reduced-motion answer
  from `RasterParams` (`Motion::Reduced` opens it paused). Space plays or pauses, `,` and `.` step a frame
  and pause. After the last run the animation holds on its last frame (`Ended`); Space plays it again. Frame
  delays at or under 10 ms are shown as 100 ms. The frames of one animation are held whole up to 256 MiB of
  RGBA8; past that the file opens as its first frame with a "too large to play" note in the Info tab. The
  export sheet and the saved copy of an animation use its first frame, and rotate and flip decline it.

**The PDF stage.** The stage machine holds the page at the top, how far down it and the zoom; the view
(`families/pdf`) holds what those point at, in a `PdfShelf` the window makes once. Each frame the view works
out the room's place in the stack of pages (`scene.rs`), asks `anyview-pdf`'s scheduler for the tiles it
shows and a margin of them, and submits the ones not held as `Job::Pdf` batches, visible before preload.
A worker draws a batch with `PdfBackend` and uploads each tile into a `TextureHandle` itself
(`work.rs`); the window turns the `Done::Pdf` into `PdfLive::received`, which holds the handle under a byte
budget, and the page boxes draw it as a `TextureLayer`. A search is a job too, and its answer is the
machine's `PdfIn::Results`. What the machine's outputs ask (scroll here, show this hit, search for this) is
left in `PdfLive::wants` for the view to carry out, because only the view knows the layout.

**The media stage.** The stage machine holds where playback is (`Playing`, `Paused`, `Scrubbing`, `Ended`)
and speaks to the player in `PlayerCommand`s; the window holds what the player last reported (position, volume,
speed, tracks, chapters, whether a picture shows, trim marks) in a `MediaShelf`, and the capsule, the
panel and the stage read it. A recording opens by starting a player (`Job::Open` calls `MediaHost::start`
through the `OpenLink`; a preload's link has none, so a neighbour never plays). The player's news arrives
as `Done::Media`, the window drains its `MediaLine` and feeds each piece to the machine as an input (the
position, the events) or to the shelf (the lists), and what the machine asks goes out through
`MediaLine::send`; the stage tells the player the size of the room (`MediaLine::resize`) whenever it
changes, since the player draws the picture into a texture of exactly that size. The capsule is
quire's, with its `Scrub` and `Level` slots: a drag on the progress bar is `ScrubStart`, `ScrubTo` and
`ScrubEnd` (quire's `Scrubber` captures the pointer, so the drag goes on outside the bar), and Esc cancels
it. A place left (`Resume::Media`) is put back by `MediaIn::Restore`, which the view sends when the
document lands, and a place the person is at is kept by the window as it moves.

Key routing is `route(key, Regions) -> Route`, not a machine: a sheet, then the palette, then the
context menu, then the global chords (⌘K, ⌘I, ⌘W, ⌘O, the Menu key and ⇧F10, Esc), then the stage,
then navigation, then the chrome. A sheet, the palette and an open context menu take every key, so
a key that means nothing to one is `Swallowed` (the menu's own arrows, Enter and letters are quire's
`Menu`, which has the keyboard while it is up; the window leaves those keys to it, and Esc closes it).
Esc undoes the innermost thing: what the stage has open, then the panel, then a quick look. The Menu
key and ⇧F10 (read as `ShortcutKey::ContextMenu` by `views/keys.rs`) open the context menu at the
middle of the content.

The right-click menu is the palette's command list under a Mac's grouping, so the menu, the palette
and the shortcuts cannot drift: `views/session.rs` builds the one `commands` list (with what the
file allows: playback, the edit offer, `FileAccess`), `context::entries` picks the rows a context
menu shows from it, and a row is run by the same `run` a palette row is. A secondary click on the
content (not on the capsule or the titlebar) sends `ContextIn::Open` with the pointer; the menu
opens only over a file that is showing and no sheet or palette. A pick runs at once, but the menu
stays in the state until quire's fade ends and it sends `Close`. The window with no file has no
context menu.

Hits of a find live with whoever searched (a document can have thousands): the stages hold the
hit count and a cursor, and ask for a hit to be shown by index. The root couples regions in three
places only: a file starting to load (the stage goes, a new ticket is issued), a palette command
(which region it belongs to), and the chrome's derived pins (a sheet, the palette or the
context menu open, media paused).

## 6. Recipes

**Add a kind of file.**
1. Add the variant to `FormatKind` (`kind/mod.rs`) and its row in `profile/table.rs`.
2. If it has formats, add a family enum implementing `Family` (`kind/<family>.rs`), a `FormatDetail`
   variant, and its lookup in `sniff/extension.rs`.
3. Add its cases to `sniff/tests.rs` and `profile/tests.rs`.

**Add a format to a family.** Add the variant with its extensions and MIME type in the family's
file; its `the_table_is_well_formed` test checks them.

**Add an action.** Add the variant to `FileAction` and its reach and binding in `action/spec.rs`;
list it in `profile/table.rs` for the kinds that offer it; extend the tables in `action/tests.rs`.
A binding that means what a standard action means uses `Binding::Standard`.

**Add a format's export.** Write its enum and its `Kind` enum in `export/<format>.rs`, implement
`ExportChoice`, and add it to the contract test in `export/tests.rs`. Name its output in
`ExportJob` payloads only if no existing job covers it.

**Add a peek.** Implement `Peek` in the back-end crate that owns the format (`anyview-image` for
images, `anyview-text` for text, code, Markdown, tables and trees) and add its arm to the registry
in `anyview-peek`, with a `From<Peeked> for Body` (a new variant of `Body` if it draws something new,
and its view in `pane`). A kind whose back end lands replaces its `FactsPeek` marker's arm with the
real type. A peek reads from the file by its `Source`, stays inside the `PeekBudget`, and
reports a count as a `Tally` when it saw only the start.

**Add a machine.** A directory in `anyview-ui` with `model.rs`, `step.rs` and `tests.rs`: an enum of
states, each variant holding only its data; `impl Machine` with an outer match on the state and an
inner one on the input, no `_` arm; a `const CASES` table of name, state, input, time, state after,
outputs; and, if it keeps a timer, a test that steps `Elapsed` at each `wake()` (`testing::settle`).
Add its region to `Viewer` and its keys to `route` if it takes any.

## 7. Tests

Pure functions get one `const CASES` table and one loop, each row named, so a failure names the
row. Stored types (`Resume`, `FileAction`, `Zoom`, `Edit`, `FilePath`, …) have a round-trip test
against the exact JSON, and a stored `Word` enum is checked against its serde form with
`ds_core::testing::word_matches_serde`. Fixtures are small byte literals of real file signatures,
in the test that uses them; where a decoder needs a real file, a fixture under
`crates/<crate>/tests/fixtures/` (each under 50 KB) is loaded through the crate's `tests/<name>/support/mod.rs`, which
builds the `Source` and sniffs it the way the viewer does (`anyview-peek` reuses the image and text
fixtures and adds one PDF). The pane is checked three ways: server-side renders kept as goldens under
`tests/snapshots/` (`DS_BLESS=1` rewrites them; read the diff), quire's stylesheet and markup lint, and a
real Blitz document through `ds-harness`, with a hybrid-painter test that reads the picture's pixels back
and skips where no GPU adapter opens. A `Peek` fake in `peek/tests.rs` shows how a generic consumer drives the
trait.

## 7b. Testing the binary

The command line, the routing of requests and the platform tasks are table tests and `#[tokio::test]`s over the
platform's fakes (`FakeInstance`, `FakeApps`, `FakeReveal`, `FakeShare`, `FakePrinter`) and a recording `Trash`:
no bus, no desktop, no real trash. `Env::isolated` stands in where the real `DbusInstance` must report that
there is no bus. `crates/anyview/tests/anyview/open_image.rs` runs a window's real root under `ds_harness`
with the runtime's pool and a desktop of fakes, opens a picture, reads back the pixels it drew and the history
file it wrote. `tests/launch.rs` holds the launch budget (ignored; FINDINGS, "The launch budget"). The media tests use the real
player on `anyview-media`'s fixtures with `ao=null` and a device with no window: `tests/anyview/media_hub.rs` (a window's
session, the desktop's entry and its controls, a session with no window, the exports and the frame),
`tests/anyview/mpris_bus.rs` (the real `MprisSession` on a private bus), `tests/media_thread.rs` (the thread the
player is polled on); they skip, saying so, where there is no graphics adapter or no `dbus-daemon`. The
fake desktop's `FakeMediaSession` has a clonable `FakeMediaHandle` to read what was published and press a
control once the session is given to the hub.

## 7a. Testing the views

The window is tested through `ds_harness` on the virtual clock (`crates/anyview-ui/tests/ui/viewer_window.rs`):
the pointer brings the chrome and rest takes it away, ⌘K lists the shared actions, the arrow keys walk the
folder; the PDF window is driven the same way (`tests/ui/pdf_window.rs`): the fixture is the one
`anyview-pdf`'s tests build in memory (`#[path]`-included, not copied), its tiles are real textures on the
harness's hybrid painter, ⌘F with the next hit moves the page, and the pixels it draws are saved when
`ANYVIEW_SHOTS` is set. Workers there run each job where it is submitted, so a result is in the mailbox by the time the
harness looks (`tests/ui/support/mod.rs`). A view component's markup is an SSR golden
(`crates/anyview-ui/tests/snapshots/`, rewritten with `DS_BLESS=1`) and the viewer's stylesheet and the
markup the harness renders go through `ds_lint`. Behaviour over time (`tests/ui/behaviour.rs`, `text_stage.rs`,
`animation.rs`) wires a window to a `Gate` that holds the jobs of chosen kinds until the test lets them go and logs
every job with its lane, a `Memory` that stands for the host's store (it hears the window's requests and answers its
reads), and the host's small pictures (`tests/ui/support/mod.rs`), so a first frame is seen while the open is still
out and a result for a file left behind is released late. The media window is driven the same way (`tests/ui/media_window.rs`) with a scripted player (`tests/ui/support/player.rs`:
a `MediaHost` that records what the window sends and says what the test makes it say, and uploads a gradient
for the picture). The registry has a test that every `FormatKind` is mapped
and agrees with `stage_support`. `ANYVIEW_SHOTS=<dir>` makes the window tests save a PNG of what they drew.

## 7d. Appearance

The one desktop appearance (`quire/appearance.toml`, the settings portal) is read and watched by the binary alone:
`host/appearance.rs` (`Appearances::follow`) loads `ds_settings::AppearanceFile` from a `Store` rooted at
`env.dirs.config`, starts `SystemPrefsWatch`, and publishes each settled change as a `Look` on a
`tokio::sync::watch`. Every window gets the receiver as a `LookFeed` root context; `ViewerApp` follows it (a window
with none keeps `Launch.look`). `anyview-ui` holds `Look` as plain data and does not name `ds-settings`. The program
never writes the file and keeps none of its own. The host (`ds-blitz`) calls `follow_root` per frame, so a scheme
switch repaints text; the viewer runs no frame loop of its own.

## 7c. Packaging (`dist/`)

`dist/org.quire.Anyview.desktop` is the desktop entry (`Exec=anyview %U`, `DBusActivatable=false`: the bus name
`org.quire.Anyview1` has its own interface, not `org.freedesktop.Application`). Its `MimeType` line is
`anyview_core::opened_mimes()`: the media types of the kinds with `StageSupport::Stage`, the one kind-to-MIME
map; `crates/anyview-core/tests/anyview-core/dist.rs` fails if the line drifts. `%U` hands the viewer `file://` URIs, which
`cli/parse.rs` decodes (another scheme or host is `CliError::NotLocal`). `dist/install.sh` and
`dist/uninstall.sh` (sharing `dist/lib.sh`; install records a receipt, uninstall removes only what it names) take `--dry-run` and `--prefix`, honour `DESTDIR`, and install the
binary, the entry, the service file (Exec rewritten to the installed binary) and the icons from
`$QUIRE_DIR/assets/icons/apps/viewer/<px>.png`; `--set-default` is opt-in; every plugin installs by default (`--without-plugin NAME` and `--no-plugins` leave them out; `--with-plugin` is an ignored
leftover; section 2m). A plugin whose tool is missing is still installed, since it greets the viewer with nothing on offer
until the tool is there. The mpv plugin is skipped with one warning, and the rest installs, when there is no git, network or build; with no `mpv` it still installs, naming `/usr/bin/mpv` (a warning says so), so the plugin works the moment mpv is installed. The helpers file `dist/helpers/anyview.toml` installs to `<prefix>/share/quire/helpers/anyview.toml`. The mpv plugin's manifest template is
`dist/plugins/anyview-mpv.toml.in`: `mpv` is the one found on the search path at install time (or `--mpv`) and the
C plugin is installed as `<prefix>/libexec/anyview/mpv-wgpu-cplugin.so`. `dev/install-test.sh` (also run by
`cargo test -p anyview-core --test anyview-core dist::`) runs both in a scratch HOME with shimmed registration tools and a shim `cargo`, and fetches mpv-wgpu only from a local repository.

## 8. Repo rules

- **Effect boundary** (`scripts/check-boundary.sh`): the tables in section 1. A pure crate that
  seems to need an effect returns a description of it to its caller.
- **Gate:**

  ```bash
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace
  ./scripts/check-boundary.sh
  cargo deny check licenses
  bash dev/install-test.sh
  bash dev/no-linked-codecs.sh        # after cargo build -p anyview: ldd shows no libmpv, no libav*
  ```

  Nothing in the workspace links libmpv or libav, so building needs neither (no `libmpv-dev`, no
  `libavcodec-dev`, no libclang for bindgen). The tests that play need the programs a person has: an `mpv` (`MPV_WGPU_MPV`,
  else the first on the search path) and mpv-wgpu's C plugin (`MPV_WGPU_CPLUGIN`, built with `cargo build -p
  mpv-wgpu-cplugin --release` in an mpv-wgpu checkout); the tests of the plugin and of the exports need `ffmpeg` and
  `ffprobe` and the plugin built by `cargo test --workspace`. Each skips, saying so, when its program is absent.

  Where the image does not have `libdav1d-dev` and `pkg-config`, `anyview-image`'s `avif` feature cannot
  build, so clippy and the tests run without `--all-features` (FINDINGS, AVIF decoding).

- **No `unsafe`** anywhere in the workspace; `unsafe_code = "deny"`. `mpv-wgpu-player` is a
  safe API and `anyview-media` needs none.
- **No `unwrap`** outside tests: clippy's `unwrap_used` is `deny`, and `clippy.toml` allows it in
  tests only.
- **Stored forms:** a type with `Serialize` is stored or crosses a wire, is adjacently tagged when
  it has data, and has a round-trip test. `FilePath`, `Mime`, `SyntaxName`, `PageRange`, `Dpi`,
  `Quality` and `Volume` validate on load, so a stored value cannot hold what a constructor
  refuses.
- **Integer units only** in data: a float appears inside arithmetic and nowhere in a type.

## 2n. The agent layer (docket)

The viewer is reachable by docket, the desktop's agent layer, through the same edge as single instance and
only with `quire-desktop` (design/36): `scripts/check-portable.sh` still builds without docket or zbus.

- **Declaration.** `dist/intents/org.quire.Anyview.toml` is docket's intents manifest (vocab 1, app
  `org.quire.Anyview`). It declares `anyview.file.open`: Read, reach `offered`, target `files` (one or several
  absolute paths). docket requires an action's name to start with the app's last name element, so it is
  `anyview.file.open`, not `file.open`. No action writes, converts or deletes; the viewer's editing and exports are
  not declared. The file is embedded in `anyview-platform` (`include_str!`) and parsed with docket-core's
  `Manifest` and `validate`, so the file the installer ships is the file the viewer answers for.
- **Serving.** `DbusInstance::claim` already owns `org.quire.Anyview1` for single instance; once it holds the name
  it calls docket-client's `serve_on` on the same connection, which exports `org.quire.IntentProvider1` and claims
  `org.quire.Anyview` (the name docket calls). `ViewerIntents::perform` turns `anyview.file.open` into the
  `Request::Open` a second launch would have forwarded, on the same channel the program already reads, so a call
  opens in the running window like any forwarded launch. Every path must be absolute and exist, or the whole call is
  refused and nothing opens. A failure to serve is logged and the viewer carries on without the agent layer.
- **Activation.** `dist/org.quire.Anyview.service` (beside `org.quire.Anyview1.service`) lets the bus start the
  viewer when the router calls `org.quire.Anyview`; the request that started it waits in the channel until the
  program reads it. `install.sh` installs the service, the manifest (`share/quire/intents/`) and the skill
  (`share/quire/skills/files-viewer/`); `uninstall.sh` removes them through the receipt.
- **Context.** The window is reported as private: the viewer holds no entities, and what is open is the person's.
- **Skill.** `dist/skills/files-viewer/` (`SKILL.md`, `skill.toml`) teaches the planner to open files with the
  action. It names only `anyview.file.open`, so it grants nothing.
- **Dependency.** docket is a git dependency at a pinned rev, and porter's `prov` and `porter-core` at the rev
  docket pins, with the same URL spelling (FINDINGS).
