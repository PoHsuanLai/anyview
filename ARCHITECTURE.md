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
| L1 | `anyview-pdf` | planned | pdfrum: open, tile scheduling, search, outline, page edits, exports |
| L1 | `anyview-media` | planned | the media player session: tracks, chapters, typed state |
| L1 | `anyview-text` | exists | text: encodings and windowed lines, code highlighting into token classes, Markdown to HTML, CSV tables, JSON trees, and the five text peeks |
| L1 | `anyview-archive` | planned | zip, tar and 7z listings and single-entry extraction |
| L1 | `anyview-font` | planned | font facts and the specimen's font face |
| L2 | `anyview-platform` | planned | the edge: traits, their Linux implementations and fakes |
| L3 | `anyview-peek` | planned | the light tier: `Peek` implementations and the pane view (what the launcher links) |
| L4 | `anyview-ui` | exists | the viewer: its pure machines (chrome, panel, palette, sheet, navigation, presentation, loading, the four stages, key routing and the root that composes them), the blocking work a worker does for it (`io`), one Dioxus view per family of formats (`families`) and the window that draws every region (`views`) |
| L5 | `anyview` | planned | the binary: launch, single instance, CLI, wiring the platform |

### Allowed edges (workspace crates and quire; everything else is forbidden)

| Crate | May depend on |
| --- | --- |
| `anyview-core` | `ds-core` (its `#[derive(Word)]` is re-exported by `ds-core`, so `ds-core-derive` is not an edge) |
| `anyview-store` | `anyview-core` |
| `anyview-ui` | `anyview-core`, `anyview-image`, `anyview-text`, `ds` (the components and hooks), `ds-blitz` (the window, `TextureLayer`), `ds-core` (the `Machine` trait and `Stamp`) |
| `anyview-image` | `anyview-core`, `ds-core` (`Word`, for the facts' labels) |
| `anyview-text` | `anyview-core`, `ds-core` (`Word` for token classes, and `base64` for `data:` URLs) |

Dev-dependencies follow the same table, plus `serde_json` for round-trip tests and `ds-core` with
its `testing` feature for `word_matches_serde` (`anyview-core`), and `tempfile` for scratch
directories (`anyview-store`, `anyview-image`, `anyview-text`).

### External boundaries (`scripts/check-boundary.sh`)

| Crate | Never reaches |
| --- | --- |
| `anyview-core` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `image`, `syntect`, `blitz-dom`, `anyrender`; `serde_json` outside tests |
| `anyview-store` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, `image`, `blitz-dom`, `blitz-paint`, `anyrender`: blocking file I/O only, so the launcher links it cheaply |
| `anyview-image` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, the `blitz-*` crates, `anyrender`, `syntect`, `pulldown-cmark`: blocking decode and encode on the caller's worker, no spawning, no clock |
| `anyview-text` | `dioxus`, `tokio`, `zbus`, `wgpu`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`, the `blitz-*` crates, `anyrender`, `image`, `resvg`, `jxl-oxide`: blocking reads on the caller's worker, no spawning, no clock |
| `anyview-peek` (planned) | `mpv-wgpu-player`, `rsmpv`, `wgpu`, `zbus`: libmpv and D-Bus stay out of the launcher's process; pdfrum only through `ds-blitz`'s `pdf` feature |
| `anyview-ui` | `zbus`, `pdfrum`, `mpv-wgpu-player`, `rsmpv`: the player and the platform reach the views as `anyview-platform` traits and `HostRequest`s, never as dependencies. `tokio` and `wgpu` arrive only through `ds-blitz`, and `image` through `anyview-image`; the library never names them. The machine modules inside it (below) stay pure: the script fails on a source file of one that names Dioxus, quire's components, a decoder, the disk, a thread or a clock |
| every crate but `anyview-platform` (planned) | `zbus`, `ashpd`, `freedesktop-*`, and the macOS and Windows bindings |

`anyview-image` depends on `image` (png and jpeg from the pinned block, gif, webp, bmp, tiff, ico, tga
and qoi added by its own manifest), `jxl-oxide`, `resvg` (without text), `kamadak-exif`, `img-parts`,
`ravif`, `thiserror` and `ds-core`. `anyview-text` depends on `syntect` (the pure-Rust regex engine, no
oniguruma), `pulldown-cmark`, `csv`, `serde_json`, `serde`, `encoding_rs`, `thiserror` and `ds-core`.

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
| `io` | whole-file reads, atomic writes, listing, removal, `stat` (private) |
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
| `navigate` | `Navigate` over the core `Sequence` |
| `presentation` | `Presentation` |
| `load` | `Load`, `Ticket` |
| `stage` | `Stage` and its four machines (`raster`, `pdf`, `media`, `text`), the shared `find` and `zoom` parts, and `dispatch`: a command or a key becomes an input for the stage that is showing |
| `keys` | `route`, `Route`, `Regions` |
| `viewer` | `Viewer`, `ViewerIn`, `ViewerOut`: the root |
| `command` | `Command` (a file action or a stage command), `StageCommand` and its keys |
| `typed` | `TypedText`: a query or a name, a static literal or typed |
| `io` | the effects: `Job` and `Done` (probe a file, open it, read a window of lines), `Workers` (the pool the binary owns), `Work`, `Reply`, `Edge` (what one window is wired to), `HostRequest` (what it asks of the binary), `Backend` and `Stop` |
| `families` | the full tier: `StageView` (one implementation per family of formats), the registry (`visit`, `family_of`, the one match on `FormatKind`), and the views `raster`, `text` and `peek_only` |
| `views` | the window: `ViewerApp`, `Launch`, the chrome, the palette, the panel, the sheets, key events as shortcuts, `stylesheet` |

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
| `decode` | `decode`, `decode_bytes`, `Decoded`, `Animation`, `Frame`, `FrameCount`, `ColourInfo`; `codec` is the one match on `RasterFormat`, `stills`, `jxl`, `svg` and `look` are private |
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
| A pure timed state machine and its time | `ds_core::machine::Machine`, `ds_core::time::stamp::Stamp` |
| When the hover chrome shows and hides, and what holds it up | `anyview_ui::Chrome`, `PinReasons` |
| Which region a key goes to | `anyview_ui::route` (`keys/route.rs`) |
| What a command or a key means to the showing stage | `Stage::input_for` (`stage/dispatch.rs`) |
| Which keys stand for a stage command | `StageCommand::from_key` (`command.rs`) |
| Ignoring a result that arrived after the person left a file | `anyview_ui::Ticket`, `Load` |
| Stepping to the next or previous find hit, wrapping | `anyview_ui::FindHits` (`stage/find.rs`) |
| The zoom a step in or out lands on, and the point it holds still | `stage/zoom.rs` (`stepped`, `centre_about`) |
| Which view shows a kind of file, and opening it | `anyview_ui::visit`, `family_of` (`families/registry.rs`) |
| What a family draws, controls and lists | `anyview_ui::StageView` (`families/view.rs`) |
| Where a picture lands in the room, and which texels show | `families/raster/geometry.rs` |
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
`anyview-peek` and the stage registry in `anyview-ui`.

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
| `Navigate` | `Idle`, `Walking { sequence }` | `Start`, `Next`, `Previous`, `First`, `Last` | `Open(path)`, `Preload(neighbours)` |
| `Presentation` | `Window`, `Peek`, `Mini`, `Background` | `ToWindow`, `ToMini` | `Become(presentation)` |
| `Load` | `Idle`, `Probing`, `Peeking { frame }`, `Opening`, `Ready`, `Failed { reason }`, each with its `Ticket` | `Begin`, `Probed`, `Peeked`, `PeekFailed`, `Opened`, `Failed` | `Probe`, `Peek`, `Open`, `Cancel`, `UseStage`, `ShowFirstFrame`, `ShowFull` |
| `RasterStage` | `Fitted`, `Zoomed`, `Panning`; an `Animation` (`Still`, `Playing`, `Paused`) rides in each | `ZoomStep`, `SetZoom`, `DoubleClick`, `PanStart`/`PanBy`/`PanEnd`, `Rotate`, `Restore`, `Animated`, `FrameTick` | `Remember`, `Turned`, `ShowFrame` |
| `PdfStage` | `Reading`, `Finding { query, hits }`, `Jumping { target }` | `Scroll`, `SetZoom`, `Find`, `Results`, `NextHit`, `GoTo`, `NextPage`, `Arrived`, `Restore` | `Remember`, `ScrollTo`, `Find(..)` |
| `MediaStage` | `Opening`, `Playing`, `Paused`, `Scrubbing { resume }`, `Ended`, `Failed` | `Player(PlayerEvent)`, `Position`, `Toggle`, `Seek*`, `Scrub*`, `SetVolume`, `Select` | `Command(PlayerCommand)`, `Buffering`, `VolumeChanged`, `TracksChanged` |
| `TextStage` | `Reading`, `Finding { query, hits }` | `Scroll`, `Find`, `Results`, `NextHit`, `ToggleSource`, `ToggleWrap`, `Restore` | `Remember`, `ScrollTo`, `Show(view)`, `Find(..)` |
| `Stage` | `NoStage`, `Raster`, `Pdf`, `Media`, `Text` | one family's input each | each family's output, lifted |
| `Viewer` | one state per region above | `Open`, a region's input, `Run` (a command from a control the window drew), `Key` | each region's output, lifted; `Probe`, `Run`, `PickFile`, `CloseWindow` |

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
UI holds only the lines it returned. `Backend` (`io/backend.rs`) is the contract of the shared work
module, declared here until that module lands.

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
in `anyview-peek`. A peek reads from the file by its `Source`, stays inside the `PeekBudget`, and
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
builds the `Source` and sniffs it the way the viewer does. A `Peek` fake in `peek/tests.rs` shows how a generic consumer drives the
trait.

## 7a. Testing the views

The window is tested through `ds_harness` on the virtual clock (`crates/anyview-ui/tests/viewer_window.rs`):
the pointer brings the chrome and rest takes it away, ⌘K lists the shared actions, the arrow keys walk the
folder. Workers there run each job where it is submitted, so a result is in the mailbox by the time the
harness looks (`tests/support/mod.rs`). A view component's markup is an SSR golden
(`crates/anyview-ui/tests/snapshots/`, rewritten with `DS_BLESS=1`) and the viewer's stylesheet and the
markup the harness renders go through `ds_lint`. The registry has a test that every `FormatKind` is mapped
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
