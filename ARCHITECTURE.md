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

## 1. Crates and allowed edges

Layers go lowest first. A crate names only crates in a lower layer, and
`scripts/check-boundary.sh` enforces the edges and the external boundaries below. A crate marked
planned has no directory yet; its row is the rule it will carry.

| Layer | Crate | Status | Purpose |
| --- | --- | --- | --- |
| L0 | `anyview-core` | exists | pure vocabulary: kinds, sniffing, units, sequence, actions, edits, exports, view memory, the `Peek` trait |
| L1 | `anyview-store` | exists | the recently-viewed history and per-file view memory on disk: one format, a read API (sill reads it) and a write API |
| L1 | `anyview-image` | exists | raster and vector images: decode to upright RGBA8, a downscaled peek with EXIF facts, encode for export, lossless JPEG rotation |
| L1 | `anyview-pdf` | exists | pdfrum: open and share a document, lay out pages, plan and draw tiles, search across the document, outline, links, page edits, exports |
| L1 | `anyview-media` | planned | the media player session: tracks, chapters, typed state |
| L1 | `anyview-text` | exists | text: encodings and windowed lines, code highlighting into token classes, Markdown to HTML, CSV tables, JSON trees, and the five text peeks |
| L1 | `anyview-archive` | planned | zip, tar and 7z listings and single-entry extraction |
| L1 | `anyview-font` | planned | font facts and the specimen's font face |
| L2 | `anyview-platform` | exists | the edge: traits, their Linux implementations and fakes |
| L3 | `anyview-peek` | exists | the light tier: the registry that maps every kind to its `Peek`, the PDF, folder and facts-only peeks, the type-erased `AnyPeeked`, and the pane view (what the launcher links) |
| L4 | `anyview-ui` | exists | the viewer: its pure machines (chrome, panel, palette, sheet, navigation, presentation, loading, the four stages, key routing and the root that composes them), the blocking work a worker does for it (`io`), one Dioxus view per family of formats (`families`: images, text, PDF pages and the facts view) and the window that draws every region (`views`) |
| L5 | `anyview` | exists | the binary: the runtime (the worker pool, the actors and delivery to the UI thread), the command line, single instance, the windows and the host that carries out what they ask through the platform |

### Allowed edges (workspace crates and quire; everything else is forbidden)

| Crate | May depend on |
| --- | --- |
| `anyview-core` | `ds-core` (its `#[derive(Word)]` is re-exported by `ds-core`, so `ds-core-derive` is not an edge) |
| `anyview-store` | `anyview-core` |
| `anyview-ui` | `anyview-core`, `anyview-image`, `anyview-pdf`, `anyview-text`, `ds` (the components and hooks), `ds-blitz` (the window, `TextureLayer`), `ds-core` (the `Machine` trait and `Stamp`) |
| `anyview-image` | `anyview-core`, `ds-core` (`Word`, for the facts' labels) |
| `anyview-text` | `anyview-core`, `ds-core` (`Word` for token classes, and `base64` for `data:` URLs) |
| `anyview-platform` | `anyview-core`, `ds-core` (`Word` for the closed vocabularies) |
| `anyview-pdf` | `anyview-core` |
| `anyview-peek` | `anyview-core`, `anyview-image`, `anyview-text`, `ds` (the pane's components), `ds-blitz` (`TextureLayer`, and the `pdf` feature's page cache) |
| `anyview` | `anyview-core`, `anyview-platform`, `anyview-store`, `anyview-ui`, `ds` (`Appearance`, `WindowHost`), `ds-blitz` (`launch`, `open_window_with`, the clipboard) |

Dev-dependencies follow the same table, plus `mpv-wgpu-player`, `wgpu` and `pollster` for `anyview`'s media-thread spike (they never reach its normal build), plus `ds-harness`, `image` and `tempfile` and `anyview-platform`'s `testing` fakes for `anyview`'s window tests, plus `serde_json` for round-trip tests and `ds-core` with
its `testing` feature for `word_matches_serde` (`anyview-core`), and `tempfile` for scratch
directories (`anyview-store`, `anyview-image`, `anyview-text`, `anyview-platform`, `anyview-peek`). `anyview-peek` also takes
`ds-harness` (a real Blitz document, and the hybrid GPU painter), `ds-lint` and `dioxus-ssr` as
dev-dependencies. `anyview-pdf` has none: its tests build their fixture in memory.

### External boundaries (`scripts/check-boundary.sh`)

| Crate | Never reaches |
| --- | --- |
| `anyview-core` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `image`, `syntect`, `blitz-dom`, `anyrender`; `serde_json` outside tests |
| `anyview-store` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `image`, `blitz-dom`, `blitz-paint`, `anyrender`: blocking file I/O only, so the launcher links it cheaply |
| `anyview-image` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, the `blitz-*` crates, `anyrender`, `syntect`, `pulldown-cmark`: blocking decode and encode on the caller's worker, no spawning, no clock |
| `anyview-text` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, the `blitz-*` crates, `anyrender`, `image`, `resvg`, `jxl-oxide`: blocking reads on the caller's worker, no spawning, no clock |
| `anyview-peek` | `mpv-wgpu-player`, `rsmpv`, `zbus`, `ashpd` anywhere in its tree: libmpv and D-Bus stay out of the launcher's process. `wgpu`, pdfrum and `tokio` are in its tree (they come with `ds-blitz`, which the launcher links) but it never names them itself: the DIRECT table of the script. Its tree is held to a package-count budget |
| `anyview-pdf` | `dioxus`, `tokio`, `zbus`, `wgpu`, `mpv-wgpu-player`, `rsmpv`, the `blitz-*` crates, `anyrender`, `image`, `resvg`, `jxl-oxide`, `syntect`, `pulldown-cmark`, `rayon`: the one crate that names pdfrum. It draws to CPU pixels with the vello-cpu rasterizer and never encodes them (`anyview-image` owns every raster encoder), spawns nothing and has no pool |
| `anyview-ui` | `zbus`, `mpv-wgpu-player`, `rsmpv`: the player and the platform reach the views as `anyview-platform` traits and `HostRequest`s, never as dependencies. It never names `pdfrum` itself either, though `pdfrum` is in its tree through `anyview-pdf`. `tokio` and `wgpu` arrive only through `ds-blitz`, `image` through `anyview-image` and `pdfrum` through `anyview-pdf` (the DIRECT table of the script); the library never names them. The machine modules inside it (below) stay pure: the script fails on a source file of one that names Dioxus, quire's components, a decoder, the disk, a thread or a clock |
| `anyview-platform` | `dioxus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `image`, the `blitz-*` crates, `anyrender`, `syntect`, `pulldown-cmark`, `resvg`, `jxl-oxide`: the edge knows the desktop, not the pictures; it spawns no thread and runs on the binary's tokio runtime |
| `anyview` | `mpv-wgpu-player`, `rsmpv` anywhere in its tree (the media actor's player is linked when `anyview-media` lands). It never names, in its own manifest, `zbus`, `ashpd`, `freedesktop-*`, `wgpu`, `pdfrum`, `image`, the `blitz-*` crates, `anyrender` or `dioxus-native` (the DIRECT table): the bus, the renderer and the decoders come through the platform and the window crates. It does name `dioxus`, for the root component every window shares, and is exempt from the "only `anyview-platform` reaches `zbus`" check for the same reason it links that crate; the DIRECT row holds it to not naming it. The runtime inside it stays generic over the back ends and names none of them |
| every crate but `anyview-platform` | `zbus`, `ashpd`, `freedesktop-*`, and the macOS and Windows bindings (the script checks the `zbus`, `ashpd` and `freedesktop` names for every crate in `crates/`) |

`anyview-image` depends on `image` (png and jpeg from the pinned block, gif, webp, bmp, tiff, ico, tga
and qoi added by its own manifest), `jxl-oxide`, `resvg` (without text), `kamadak-exif`, `img-parts`,
`ravif`, `thiserror` and `ds-core`. `anyview-text` depends on `syntect` (the pure-Rust regex engine, no
oniguruma), `pulldown-cmark`, `csv`, `serde_json`, `serde`, `encoding_rs`, `thiserror` and `ds-core`.

`anyview-peek` depends on the three crates below it, `ds`, `ds-blitz` (feature `pdf`), `dioxus` and
`thiserror`. `wgpu` is not an exception to its rule so much as a fact of `ds-blitz`: `TextureLayer` and the PDF
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
| `units` | page, media, ratio, zoom, turn, content-space and pixel newtypes; all integer |
| `source` | `FilePath`, `FileName`, `FileStamp`, `Source` |
| `kind` | `FormatKind`, `Mime`, `FormatDetail`, `SyntaxName` and the format families |
| `sniff` | `sniff`, `sniff_zip`, `Sniffed` and the head and entries they read |
| `sequence` | `NonEmpty`, `Sequence`, `moved`, `neighbours` |
| `edit` | `Edit`, `EditKind` |
| `action` | `FileAction`, `Reach`, `reach`, `shortcut` |
| `export` | the per-format export enums, `ExportChoice`, `ExportJob` and its payloads |
| `resume` | `Resume`, `TrackChoice` |
| `facts` | `FactLabel`, `FactValue`, `Facts` |
| `peek` | `Peek`, `PeekBudget`, `StageSupport` |
| `work` | `Backend`, `Stop`, `StopState`, `Ticket`, `Ticketed`: the contract with the threads. The one public module: reached as `anyview_core::work::X` |
| `profile` | the one match on `FormatKind`: `actions_for`, `edits_for`, `stage_support` |

## 2a. Modules inside `anyview-store`

Same rules as section 2: private modules, each public item re-exported once at the crate root.
`io` is the only module that touches the disk; the others are pure.

| Module | Holds |
| --- | --- |
| `error` | `StoreError`, `StoreOp` |
| `viewed` | `Viewed`, seconds since the epoch, handed in by the caller |
| `label` | `ResumeLabel` and `resume_label`, the row subtitle derived from a `Resume` |
| `history` | `HistoryCap`, `HistoryEntry`, `History` and the pure `history_after_view` |
| `record` | the per-file record, its hashed file name, `applicable` and `prune_decision` (private) |
| `io` | the effects: `Job` and `Done` (probe a file, make its first frame, open it, read a window of lines, search it, open a neighbour ahead, read a stamp, list a folder), `Workers` (the pool the binary owns), `Work` (with its `WorkLane` and `WorkKind`), `Reply`, `Edge` (what one window is wired to), `HostRequest` (what it asks of the binary), `ResumeSource` and `FirstFrameSource` (what the binary lends it to read), `folder_sequence`, `Backend` and `Stop` |
| `reader` | `read_history`, `HistoryRead`: the API the launcher links |
| `writer` | `StoreWriter`: `record_view`, `save_resume`, `load_resume` |

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
| `sheet` | `Sheet`, `ExportDraft` (one format's export choice) |
| `navigate` | `Navigate` over the core `Sequence`; `Leave` ends a walk when a dropped file is not one of the list |
| `presentation` | `Presentation` |
| `load` | `Load`, `Ticket`, `freshness` (whether a file on disk is still the one opened: the decision behind a reload) |
| `stage` | `Stage` and its four machines (`raster`, `pdf`, `media`, `text`), the shared `find` and `zoom` parts, `dispatch` (a command or a key becomes an input for the stage that is showing) and `resume` (the place a stage keeps, and the input that puts one back) |
| `keys` | `route`, `Route`, `Regions` |
| `viewer` | `Viewer`, `ViewerIn`, `ViewerOut`: the root |
| `command` | `Command` (a file action or a stage command), `StageCommand` and its keys |
| `typed` | `TypedText`: a query or a name, a static literal or typed |
| `io` | the effects: `Job` and `Done` (probe a file, open it, read a window of lines, draw tiles of a PDF), `WorkLane` (how soon a job is wanted: `Job::lane` is its one decision), `Workers` (the pool the binary owns), `Work`, `Reply`, `Edge` (what one window is wired to), `HostRequest` (what it asks of the binary), `Backend` and `Stop` |
| `families` | the full tier: `StageView` (one implementation per family of formats), the registry (`visit`, `family_of`, the one match on `FormatKind`), the views `raster`, `text`, `pdf` and `peek_only`, and `find_bar` (the one find bar, which `text` and `pdf` wrap with their own machine's inputs) |
| `views` | the window: `ViewerApp`, `Launch`; `window` (the component), `shelf` (the results the window holds, and `Dispatch`), `carry` (what each output of the root does), `arrive` (each result of a worker as an input), `effects` (what waits on a probe or the device), `preloads` (the files opened ahead); the chrome, the palette, the panel, the sheets, key events as shortcuts, `stylesheet` |

## 2c. Modules inside `anyview-image`

Same rules as section 2: private modules, each public item re-exported once at the crate root.
Everything is blocking and runs on the caller's worker: no runtime, no spawning, no clock. `decode`
and `decode_bytes` are the one way pixels come out, and `encode` the one way they go in.

| Module | Holds |
| --- | --- |
| `error` | `ImageError` |
| `pixels` | `Rgba8` (straight alpha), `PremultipliedRgba8`, and the one conversion between them |
| `orientation` | `ExifOrientation` (a `Mirror` then a clockwise `QuarterTurn`), its tag table and `applied` |
| `exif` | `ExifFacts`, `Exposure`, `Ratio`: read with `kamadak-exif`; `format` words them; `patch` writes the orientation entry (private) |
| `scale` | `resized` (the export's `Resize`); peek-budget fitting (private) |
| `decode` | `decode`, `decode_bytes`, `declared_size` (the upright size from the header and EXIF alone), `Decoded`, `Animation`, `Frame`, `FrameCount`, `ColourInfo`; `codec` is the one match on `RasterFormat`, `stills`, `jxl`, `svg` and `look` are private |
| `peek` | `RasterPeek` and `VectorPeek` (the two `Peek` implementations), `ImagePeek`, `PeekedFormat` |
| `encode` | `encode`, `encode_bmp`, `encode_with_metadata`; `codecs`, `avif` and `metadata` (EXIF and ICC splicing with `img-parts`) are private |
| `rotate` | `rotate_jpeg`: lossless rotation by rewriting the EXIF orientation segment |

**Alpha** is straight (not premultiplied) everywhere in this crate; `Rgba8::premultiplied` is the
conversion a GPU compositor needs, and `PremultipliedRgba8` is a distinct type so the two cannot be
mixed. **Orientation** is applied in the decoder: pixels come back upright and nothing downstream
reads the EXIF tag. Encoding takes upright pixels, so a carried EXIF block has its orientation
reset to upright. A JPEG rotated "in place" is not decoded at all: `rotate_jpeg` rewrites one APP1
segment and copies every other byte.

The `avif` feature adds AVIF decoding through the `image` crate and the dav1d C library (BSD-2,
linked dynamically, found with pkg-config). It is off by default, so the default build needs no C
toolchain pieces; without it an AVIF file is `ImageError::NotCompiledIn`. AVIF encoding is `ravif`
(pure Rust) and is always built.

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
| `markdown` | `render`, `Rendered`, `RenderEnv`, `LocalFiles`, `NoFiles`, `Heading`, `HeadingLevel`, `Anchor`; `events` (the safety pass), `images`, `links` and `outline` are private |
| `table` | `Table`, `HeaderMode`, `RowCount`, `RowIndex`, `ColumnCount`; `header` (the guess) is private |
| `tree` | `Tree`, `TreePath`, `TreeRow`, `RowLabel`, `NodeKind`, `ChildCount`; `node` and `rows` are private |
| `peek` | `PlainPeek`, `CodePeek`, `MarkdownPeek`, `TablePeek`, `TreePeek` and their `*Peeked` types, `Tally`, `PEEK_LINES` |
| `escape` | HTML escaping, the one place it is written (private) |

Token classes are words (`keyword`, `string`, `comment`, …), never colours: the stylesheet maps each
`tok-<class>` to a design-system colour token. Markdown never passes raw HTML through (it is shown
as text), keeps only web, mail and relative link targets, and writes local images as `data:` URLs
read through `LocalFiles`, because quire's sealed frames load nothing else.

## 2e. Modules inside `anyview-platform`

Same rules as section 2: private modules, each public item re-exported once at the crate root; the
implementations are reached through `linux` and `testing`. A second platform adds `macos/` or
`windows/` beside `linux/` and selects it in `lib.rs`; no other crate changes. Nothing reads
`std::env`, `dirs` or a bus address outside `env`: the binary builds an `Env` (`Env::from_process`),
a test builds its own (`Env::isolated`, which names no bus and refuses to start programs). Async
methods are driven by the caller's runtime and never spawn; blocking ones (Open With, thumbnails)
run on the caller's worker.

| Module | Holds |
| --- | --- |
| `error` | `PlatformError` (`NoBus`, `Bus`, `Io`, `Thumbnail`, `Spawn`, `Exec`), `IoOp` |
| `env` | `Env` (`dirs`, `session`, `spawn`), `Dirs`, `BusRoute` (`Usual`, `Address`, `Absent`) |
| `spawn` | `Argv`, the `Spawn` trait, `ProcessSpawn`, `RefuseSpawn` |
| `uri` | `file_uri`: the escaped `file://` URI the thumbnail spec hashes and the file manager takes |
| `instance` | `Instance`, `Request` (`Open`, `Peek`, `Play`), `Claim`, `Primary` |
| `media` | `MediaSession`, `MediaState`, `MediaControl`, `PlaybackStatus`, `Ability`, `SeekDirection`, `TrackSerial` |
| `apps` | `AppsForType`, `AppEntry`, `DesktopId`, `Association` |
| `thumbnail` | `ThumbnailCache`, `ThumbSize`, `ThumbPixels` |
| `printer` | `Printer`, `PrintOutcome` (`Printed`, `Cancelled`, `NoDialog`), `JobTitle` |
| `share` | `Share`, `ShareTarget` |
| `reveal` | `Reveal` |
| `stacking` | `WindowStacking`, `Stacking`, `StackingOutcome` |
| `linux` | one implementation per trait: `DbusInstance`, `MprisSession`, `DesktopApps`, `FreedesktopThumbnails`, `PortalPrinter`, `MailShare`, `FileManagerReveal`, `NoStacking` |
| `testing` (feature `testing`) | `FakeInstance`, `FakeMediaSession`, `FakeApps`, `FakeThumbnails`, `FakePrinter`, `FakeShare`, `FakeReveal`, `FakeStacking`, `RecordingSpawn`; clones share their record |

The trait shapes (a trait whose method awaits returns `impl Future + Send`, so a consumer is
generic over it rather than holding a `dyn`):

| Trait | Methods |
| --- | --- |
| `Instance` | `claim(&Request) -> Result<Claim>`: own `org.quire.Anyview1` (`Claim::Primary`, whose `Primary::next` yields what later launches forwarded) or forward the request to the owner (`Claim::Forwarded`) |
| `MediaSession` | `publish(&MediaState) -> Result<()>`; `next_control() -> Option<MediaControl>` |
| `AppsForType` | `apps_for(&Mime) -> Vec<AppEntry>` (default first); `open_with(&DesktopId, &FilePath) -> Result<()>` |
| `ThumbnailCache` | `lookup(&FilePath, &FileStamp, ThumbSize) -> Result<Option<ThumbPixels>>`; `store(.., &ThumbPixels) -> Result<()>` |
| `Printer` | `print(&[u8], &JobTitle) -> Result<PrintOutcome>` |
| `Share` | `targets() -> Vec<ShareTarget>`; `share(&FilePath, ShareTarget) -> Result<()>` |
| `Reveal` | `reveal(&FilePath) -> Result<()>` |
| `WindowStacking` | `request(Stacking) -> StackingOutcome` |

On the bus: `org.quire.Anyview1` at `/org/quire/Anyview1` has `Open(as)`, `Peek(s)` and `Play(s)`
over absolute paths (a relative one is an `InvalidArgs` error), and `dist/org.quire.Anyview1.service`
is the activation file that starts `anyview` when a call arrives while none runs. The player is
`org.mpris.MediaPlayer2.anyview` at `/org/mpris/MediaPlayer2`; its track id is
`/org/quire/Anyview1/Track/<TrackSerial>`. Thumbnails live at
`<cache>/thumbnails/{normal,large,x-large}/<md5 of the file URI>.png` with `Thumb::URI`,
`Thumb::MTime` and `Thumb::Size` text chunks. The tests of the bus implementations run against a
`dbus-daemon` the test starts with its own configuration (no service directories) and skip with a
message when the program is not installed.

`anyview-platform` depends on `zbus` (its `tokio` feature, so the binary's runtime drives it),
`freedesktop-desktop-entry`, `tokio` (channels only), `md-5`, `png`, `percent-encoding`, `memfd`,
`futures-util`, `dirs`, `thiserror`, `anyview-core` and `ds-core`.
## 2f. Modules inside `anyview-peek`

Same rules as section 2: private modules, each public item re-exported once at the crate root. The peeks
are blocking and run on the caller's worker; only `pane` draws.

| Module | Holds |
| --- | --- |
| `error` | `PeekError` |
| `registry` | `KindVisitor`, `visit`: the one exhaustive match over `FormatKind` in the light tier |
| `body` | `Body` (the type-erased result), `Light` (a `Peek` whose result and error convert into `Body` and `PeekError`) |
| `any` | `AnyPeeked` and `peek`: runs the registry's visitor, adds the size and the date, and turns a failure into `Body::Unavailable` |
| `described` | `FactsPeek<K>` and one marker per kind with no back end yet (`VideoPeek`, `AudioPeek`, `FontPeek`, `ArchivePeek`, `BookPeek`, `OfficePeek`, `OtherPeek`): what sniffing established, nothing pretended |
| `folder` | `FolderPeek`, `FolderSummary`: one level, item count, size and kinds |
| `pdf` | `PdfPeek`: the first page, through `ds-blitz`'s thumbnail cache |
| `when` | `modified_text`: a modification time as UTC |
| `pane` | `Pane`, `STYLE`; `picture` (a `TextureLayer`), `lines` (plain and highlighted), `grid` (a table and a tree's top level, both as quire's `Table`) and `frame` (Markdown in a sealed frame) are private, and `pane.css` is its stylesheet |

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
| `export` | `ExportPiece`, `plan_export`, `selected`, `write_pages`, `write_text` |
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
| `program` | `run`; `claim_role` and `Role` (`Forwarded`, `Primary`, `Alone`: single instance over the `Instance` trait); `relay`, `open_each` and `files_of` (what the viewer's name receives after the first window); `WARM_FOR` |
| `seam` | `Workforce`: the `Pool`, the `Runner` for the views' `Work` and the `Mailbox` its endings come back through; `NoticeWaker`, `Notice`. The one implementation of `anyview_ui::Workers` |
| `host` | `route` (a `HostRequest` as a `Carry`: the window's own `WindowTask`, the desktop's `Task`, or a `Declined` with its reason; pure), `Shown` (the file a window shows), `Desktop` and the `Hosting` trait (the tasks carried out through the platform's traits), `LinuxDesktop`, `Trash` with `SystemTrash`, `Store` (the one writer of the history, behind a lock) and the `Clock`, `Outcome` and `report` |
| `window` | `Opening` (a file and its folder's sequence), `Factory` and `Seed` (what every window shares, and what makes one window its own), `first_root` and `Inbox` (the root of the first window, which also opens the windows that are asked for later) |

A window's `HostRequest`s go from its `Edge` over a channel to a task of its root component, which routes each
and either does it itself (closing the window, the clipboard: only that thread can) or hands the task to
`Hosting::carry_out`, which runs it on the platform runtime and resolves with an `Outcome`. A second launch is
claimed by the first viewer over D-Bus; its files become `Opening`s (the folder is listed on the blocking pool),
which the first window's root receives and turns into `ds_blitz::open_window_with` calls on the same event loop.

## 3. Layer rules

1. **A lower layer never names a higher one.** If something needed lives above, move the shared
   piece down or pass it in.
2. **`anyview-core` is pure.** Effects (reading files, listing a zip, decoding, playing) belong to
   crates above it, which hand it values: the first 4 KiB of a file, a zip's entry names.
3. **Every kind of file is mapped from day one.** A kind without a viewer stage is `PeekOnly` (facts
   plus Open With…), never a stub.
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
| The "Page 143" a history row shows | `anyview_store::resume_label` |
| What each format exports, and the sheet's contract | `anyview_core::ExportChoice` and the per-format enums (`export`) |
| The shared encoders an export becomes | `anyview_core::ExportJob` |
| Rows of facts a pane lists | `anyview_core::Facts` |
| The light tier of a format | `anyview_core::Peek` |
| Which peek a kind has (the light tier's one match on `FormatKind`) | `anyview_peek::visit`, `KindVisitor` (`registry.rs`) |
| A peek's result with its type erased, and the rows beside it | `anyview_peek::AnyPeeked`, `peek`, `Body` |
| What a kind with no back end yet shows | `anyview_peek::FactsPeek` (`described.rs`) |
| A folder's count, size and kinds | `anyview_peek::FolderPeek` |
| The first page of a PDF for a pane | `anyview_peek::PdfPeek`, over `ds_blitz::pdf_thumb_blocking` |
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
| What a PDF export is made of | `anyview_pdf::plan_export`, `ExportPiece` |
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
| Being the viewer, or forwarding a launch to it | `anyview::program::claim_role` over `anyview_platform::Instance` |
| What a window's request means to the host | `anyview::host::route` |
| Carrying out Open With, reveal, share, print, trash, rename, duplicate, the history | `anyview::host::Desktop` (`Hosting::carry_out`) |
| Moving a file to the trash | `anyview::host::Trash`, `SystemTrash` |
| The one writer of the history and view memory, and the time it stamps | `anyview::host::Store`, `Clock` |
| A window of the viewer, the first and the later ones | `anyview::window::first_root`, `Seed` |
| Stepping to the next or previous find hit, wrapping | `anyview_ui::FindHits` (`stage/find.rs`) |
| A phrase found in a text, as hits that cut a line at character boundaries | `anyview_text::Needle`, `FindHit`, `TextLines::find` |
| The find bar: field, standing, steps (a stage wraps it with its own machine's inputs) | `families/find_bar.rs` (`FindBar`, `FindStep`) |
| How soon a job is wanted, and which pool lane that is | `Job::lane` -> `WorkLane`; the binary's one mapping to the runtime's `Lane` (`seam/workforce.rs`) |
| Which hits are on a line, how a line is cut at them, where the view scrolls to show one | `FoundHits`, `pieces`, `top_for` (`families/text/find.rs`) |
| How many lines fit a page when long lines wrap | `families/text/wrap.rs` |
| Where a key step through a text lands | `stage/text/steps.rs` |
| What a stage remembers of where the person is, and puts back | `Stage::resume`, `Stage::restoring` (`stage/resume.rs`) |
| Where a file was left, read by the window | `anyview_ui::ResumeSource` (the binary implements it over `anyview_store`) |
| Where a file is left, kept | `HostRequest::Remember` (the binary writes it through `anyview_store`) |
| The cheap first frame of a file | `StageView::first_frame` (`families/view.rs`); a picture's from `anyview_ui::FirstFrameSource` (the host's thumbnail cache), an animation's or a vector's from `anyview_image`'s peeks, a text's the first bytes of the file (`families/text/doc.rs`) |
| The size a picture will have, without decoding it | `anyview_image::declared_size` |
| The files opened ahead, and the one just left | `views/preloads.rs` |
| The folder of a file as the list the arrow keys walk, in name order | `anyview_ui::folder_sequence` (`io/folder.rs`) |
| Whether a changed file is reloaded | `anyview_ui::freshness` (`load/fresh.rs`); the host says a file changed through `Edge::changed` |
| The frames of an animation, and the clock that plays them | `RasterDoc` strip (`families/raster/doc.rs`), `use_frame_clock` (`families/raster/view.rs`) |
| The zoom a step in or out lands on, and the point it holds still | `stage/zoom.rs` (`stepped`, `centre_about`) |
| The person's directories, the session bus and starting a program | `anyview_platform::Env` (`env.rs`); nothing else reads `std::env`, `dirs` or a bus address |
| One viewer process, and forwarding a launch to it | `anyview_platform::Instance`, `Request` |
| Now playing and the desktop's media controls | `anyview_platform::MediaSession`, `MediaState`, `MediaControl` |
| Which applications open a type, and opening with one | `anyview_platform::AppsForType` |
| The shared thumbnail cache, and a file's `file://` URI | `anyview_platform::ThumbnailCache`, `file_uri` |
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

## 5. The canonical traits

A trait exists where two or more implementations swap or a generic consumer runs over many
(CONVENTIONS section 5). Closed sets stay enums.

```rust
/// The light tier of one format: cheap, no GPU, no libmpv. One implementation per kind.
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
map to `PeekOnlyStageView` (facts and Open With…, a real view). `anyview_core::stage_support` says the same in
the core's table, and a test holds the two equal. `Workers` (`io/workers.rs`) is the one seam to the
binary's pool: `fn submit(&self, work: Work)`.

## 5b. Threads

The binary owns every thread; libraries never spawn. Back-end crates expose `Backend::run` and
their work items and nothing else.

| Thread | Owner | Runs |
| --- | --- | --- |
| UI | the window (`ds-blitz`) | the machines, the views, `TextureLayer`, Markdown and HTML layout; never blocks |
| workers, `PoolSize::from_cores(cores)` (cores minus one, at least one) | `runtime::Pool` | back-end jobs, visible-lane first |
| `anyview-media` | `runtime::Actor` | the media player: built, polled and commanded only there |
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
go out through an `Outbox`. The player runs this way: `poll` on the media thread is checked in
`crates/anyview/tests/media_thread.rs` (FINDINGS).

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
| `Sheet` | `Closed`, `Export { draft }`, `ConfirmTrash`, `Rename { name }` | `OpenExport`, `AskTrash`, `AskRename`, `PickKind`, `Change`, `Typed`, `Confirm`, `Cancel` | `Opened`, `Closed`, `Export`, `Trash`, `Rename` |
| `Navigate` | `Idle`, `Walking { sequence }` | `Start`, `Next`, `Previous`, `First`, `Last`, `Leave` | `Open(path)`, `Preload(neighbours)` |
| `Presentation` | `Window`, `Peek`, `Mini`, `Background` | `ToWindow`, `ToMini` | `Become(presentation)` |
| `Load` | `Idle`, `Probing`, `Peeking { frame }`, `Opening`, `Ready`, `Failed { reason }`, each with its `Ticket` | `Begin`, `Probed`, `Peeked`, `PeekFailed`, `Opened`, `Failed` | `Probe`, `Peek`, `Open`, `Cancel`, `UseStage`, `ShowFirstFrame`, `ShowFull` |
| `RasterStage` | `Fitted`, `Zoomed`, `Panning`; an `Animation` (`Still`, `Playing`, `Paused`) rides in each | `ZoomStep`, `SetZoom`, `DoubleClick`, `PanStart`/`PanBy`/`PanEnd`, `Rotate`, `Restore`, `Animated`, `FrameTick` | `Remember`, `Turned`, `ShowFrame` |
| `PdfStage` | `Reading`, `Finding { query, hits }`, `Jumping { target }` | `Scroll`, `SetZoom`, `Find`, `Results`, `NextHit`, `GoTo`, `NextPage`, `Arrived`, `Restore` | `Remember`, `ScrollTo`, `Find(..)` |
| `MediaStage` | `Opening`, `Playing`, `Paused`, `Scrubbing { resume }`, `Ended`, `Failed` | `Player(PlayerEvent)`, `Position`, `Toggle`, `Seek*`, `Scrub*`, `SetVolume`, `Select` | `Command(PlayerCommand)`, `Buffering`, `VolumeChanged`, `TracksChanged` |
| `TextStage` | `Reading`, `Finding { query, hits }` | `Scroll`, `Step` (a line, a page, the start, the end), `Find`, `Results`, `NextHit`, `ToggleSource`, `ToggleWrap`, `Restore` | `Remember`, `ScrollTo`, `Show(view)`, `Find(..)` |
| `Stage` | `NoStage`, `Raster`, `Pdf`, `Media`, `Text` | one family's input each | each family's output, lifted |
| `Viewer` | one state per region above | `Open`, `Reload` (a changed file: the stage stays), `Dropped` (the first file opens; one's folder or the several are the list), a region's input, `Run` (a command from a control the window drew), `Key` | each region's output, lifted; `Probe`, `Reload`, `ListFolder`, `Run`, `PickFile`, `CloseWindow` |

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
  `Peeking` state counts it. A picture's first frame is the host's thumbnail when `FirstFrameSource` has one
  (shown in the box `declared_size` says the picture will fill), otherwise the first frame of a GIF or WebP or a
  drawn-small SVG; a large text's is its first 256 KiB.
- *Neighbours.* Each `Navigate` move says `Preload(neighbours)`; the window opens those two files on the
  `WorkLane::Preload` lane into textures of their own (`Job::Preload`) and keeps the file it just left as it was
  left. Arriving at a held file shows it with no probe and no open, and reads its stamp (`Job::Stat`) to see that
  it is still current.
- *Resume.* `Job::Probe` asks the host's `ResumeSource` for where the file was left (`Probed::resume`); after the
  load machine installs the stage, `Stage::restoring` turns it into the stage's own `Restore` input. Every
  settled gesture says `HostRequest::Remember`, which is how the host keeps it.
- *Reload.* The host says a file changed (`Edge::changed`); the window reads its stamp and, if `freshness` says it
  differs from the one opened, sends `ViewerIn::Reload`. The root probes again with the stage left in place, a
  probe of the same family keeps it, and what is on screen stays until the new copy lands. The window asks the
  host to watch the shown file (`HostRequest::Watch`, replaced by the next); the watcher itself is the binary's,
  so no crate here names `notify`.
- *Drop.* The window is a drop target (`ds::file_drop`); `ViewerIn::Dropped` opens the first file, ends the walk,
  and asks for the folder as the new list (`ViewerOut::ListFolder`, answered through `folder_sequence`).
- *Animation.* An open that finds frames uploads each into a texture of its own; the stage machine's `Animation`
  says which is on screen and a clock in the view (`use_frame_clock`) sends `FrameTick` when the current
  frame's delay is up.

**The PDF stage.** The stage machine holds the page at the top, how far down it and the zoom; the view
(`families/pdf`) holds what those point at, in a `PdfShelf` the window makes once. Each frame the view works
out the room's place in the stack of pages (`scene.rs`), asks `anyview-pdf`'s scheduler for the tiles it
shows and a margin of them, and submits the ones not held as `Job::Pdf` batches, visible before preload.
A worker draws a batch with `PdfBackend` and uploads each tile into a `TextureHandle` itself
(`work.rs`); the window turns the `Done::Pdf` into `PdfLive::received`, which holds the handle under a byte
budget, and the page boxes draw it as a `TextureLayer`. A search is a job too, and its answer is the
machine's `PdfIn::Results`. What the machine's outputs ask (scroll here, show this hit, search for this) is
left in `PdfLive::wants` for the view to carry out, because only the view knows the layout.

Key routing is `route(key, Regions) -> Route`, not a machine: a sheet, then the palette, then the
global chords (⌘K, ⌘I, ⌘W, ⌘O, Esc), then the stage, then navigation, then the chrome. A sheet
and the palette take every key, so a key that means nothing to one is `Swallowed`. Esc undoes the
innermost thing: what the stage has open, then the panel, then a quick look.

Hits of a find live with whoever searched (a document can have thousands): the stages hold the
hit count and a cursor, and ask for a hit to be shown by index. The root couples regions in three
places only: a file starting to load (the stage goes, a new ticket is issued), a palette command
(which region it belongs to), and the chrome's derived pins (a sheet or palette open, media
paused).

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
`crates/<crate>/tests/fixtures/` (each under 50 KB) is loaded through `tests/support/mod.rs`, which
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
there is no bus. `crates/anyview/tests/open_image.rs` runs the first window's real root under `ds_harness`
with the runtime's pool and a desktop of fakes, opens a picture, reads back the pixels it drew and the history
file it wrote. `tests/launch.rs` holds the launch budget (ignored; FINDINGS, "The launch budget").

## 7a. Testing the views

The window is tested through `ds_harness` on the virtual clock (`crates/anyview-ui/tests/viewer_window.rs`):
the pointer brings the chrome and rest takes it away, ⌘K lists the shared actions, the arrow keys walk the
folder; the PDF window is driven the same way (`tests/pdf_window.rs`): the fixture is the one
`anyview-pdf`'s tests build in memory (`#[path]`-included, not copied), its tiles are real textures on the
harness's hybrid painter, ⌘F with the next hit moves the page, and the pixels it draws are saved when
`ANYVIEW_SHOTS` is set. Workers there run each job where it is submitted, so a result is in the mailbox by the time the
harness looks (`tests/support/mod.rs`). A view component's markup is an SSR golden
(`crates/anyview-ui/tests/snapshots/`, rewritten with `DS_BLESS=1`) and the viewer's stylesheet and the
markup the harness renders go through `ds_lint`. Behaviour over time (`tests/behaviour.rs`, `text_stage.rs`,
`animation.rs`) wires a window to a `Gate` that holds the jobs of chosen kinds until the test lets them go and logs
every job with its lane, a `Memory` that stands for the host's store (it hears the window's requests and answers its
reads), and the host's small pictures (`tests/support/mod.rs`), so a first frame is seen while the open is still
out and a result for a file left behind is released late. The registry has a test that every `FormatKind` is mapped
and agrees with `stage_support`. `ANYVIEW_SHOTS=<dir>` makes the window tests save a PNG of what they drew.

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
  ```

  Where the image does not have `libdav1d-dev` and `pkg-config`, `anyview-image`'s `avif` feature cannot
  build, so clippy and the tests run without `--all-features` (FINDINGS, AVIF decoding).

- **No `unsafe`** anywhere in the workspace; `unsafe_code = "deny"`.
- **No `unwrap`** outside tests: clippy's `unwrap_used` is `deny`, and `clippy.toml` allows it in
  tests only.
- **Stored forms:** a type with `Serialize` is stored or crosses a wire, is adjacently tagged when
  it has data, and has a round-trip test. `FilePath`, `Mime`, `SyntaxName`, `PageRange`, `Dpi`,
  `Quality` and `Volume` validate on load, so a stored value cannot hold what a constructor
  refuses.
- **Integer units only** in data: a float appears inside arithmetic and nowhere in a type.
