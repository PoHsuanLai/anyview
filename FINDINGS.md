# Findings

This file holds two kinds of entry: **open items**, things still unresolved with the condition
that ends each, and **standing facts**, things true at the current pins that code and tests rely
on. It is a reference, not a log: how each was found lives in git history.

## Open items

- **quire's `PreviewPane` cannot hold anyview's pane.** It takes data, `PaneContent` (`Image`, `Text`, `Pdf`,
  `App`, `Facts`, `Emoji`, `Web`), not an element, and draws its own media, caption and actions column.
  `anyview_peek::Pane` draws the media, the name and the facts itself, so sill (phase C) cannot put it
  inside `PreviewPane` to keep its action row, its cue, its pending look and its slide-in. Ends when quire
  gives `PreviewPane` a slot (`PaneContent::Slot(Element)`) or splits its frame (the actions, the cue and
  the entrance) from its content; then `Pane` goes in the slot. Until then the launcher can lay `Pane` out
  beside its own actions.
- **quire's `PdfPage::Ready` carries a PNG `data:` URL.** The PDF peek goes through `ds_blitz::pdf_thumb_blocking`
  (the cache the launcher already uses), whose page is `ImageSource`, so a PDF's first page is the one
  picture here that is not a `TextureLayer`. Ends when quire's `PdfPage::Ready` can hold pixels for a
  `TextureLayer` (a `TextureHandle` or a `Pixels`); then `PdfPeek` hands them over and the pane draws them
  the way it draws any picture.
- **quire's markup lint flags quire's own `TextureLayer`.** The `object.ds-texture-layer` it renders has a
  class no stylesheet rule defines (it styles itself inline), so `ds_lint::markup` reports
  `UnstyledClass` for any page that shows one. `tests/pane.rs` skips exactly that selector. Ends when quire
  gives the class a rule or the lint an allowance for it.
- **A PDF's page count and title are not known to a peek.** pdfrum is reached only through `ds-blitz`, whose
  thumbnail answers the first page's raster and size; the facts list the page size in points, and the
  Pages and Title rows wait for `anyview-pdf`. A PDF is read whole by `ds-blitz`, so `PdfPeek` refuses one
  longer than the budget's bytes (`PeekError::OverBudget`): the launcher's budget must cover the PDFs it
  wants to show. Ends when `anyview-pdf` supplies a peek that reads only the first page (the registry arm
  names it instead).
- **A modification time is shown in UTC.** `modified_text` reads no zone: this crate has no clock and no
  zone database, and `jiff` is not in its tree. Ends when `anyview-platform` can hand a peek the person's
  zone; the row then moves into `anyview-core`'s `FactValue` with a zone argument.
- **The kinds with no back end show only what sniffing says.** Video, audio, fonts, archives, books, office
  documents and unknown files are `FactsPeek`: the type, the size and the date, with no duration, no
  listing, no cover and no specimen. Each ends when its crate lands (`anyview-media`, `anyview-font`,
  `anyview-archive`) and the registry's arm names the real peek. A book or office file that is a zip is
  an archive here, because opening the zip is the archive crate's job.
- **A folder summary is one level deep.** `FolderPeek` counts the first 10,000 non-hidden entries, adds up the
  size of the files directly inside it (not the contents of folders), and sniffs the first files the byte
  budget pays for (4 KiB each), so on a large folder the named kinds are those of the first files by name.
  Ends if a recursive size matters: it needs a deadline the peeks do not have yet (`PeekBudget::time` is
  not enforced).
- **The pane has one width.** Its media box is quire's `PANE_MEDIA` (328 by 220 px), a table shows at most six
  columns and forty rows, and code and plain lines are clipped, not wrapped or scrolled. It is a glance for
  a 360 px pane; the viewer's stages are the full views. Ends when the launcher's pane gets other widths:
  the media box then reads its parent's size.
- **The Markdown start is shown on a white sheet.** The peek's HTML goes into an `<iframe srcdoc>` whose own
  stylesheet has literal colours, because a frame cannot read the pane's tokens; it is dark on white in both
  schemes, as a mail's original message is (`--foreign-ground`). Ends when the Markdown stage's stylesheet
  lands in `anyview-ui` and the frame takes the same one (see the highlighted fences item above).
- **Each `Pane` writes its stylesheet.** `Pane` draws `AppStyle { css: STYLE }` itself, so the markup holds
  one `<style>` per pane. Ends if the launcher wants it once for the window: it can draw `AppStyle` with
  `anyview_peek::STYLE` at its root and the pane's copy is then redundant but harmless.
- **The picture needs the window's GPU.** `TextureLayer` draws nothing on the CPU painter and until the
  device arrives with the first frame; the pane shows a placeholder block meanwhile and for good on a CPU
  renderer. Ends if a CPU-only launcher matters: it then needs a fallback (a PNG `data:` URL from
  `anyview_image::encode`), which this crate does not carry.
- **`NoExportKind` is a hand-written `Word`.** `#[derive(Word)]` refuses an enum with no variant,
  and `ExportChoice::Kind` must be a `Word` even for a format with no export. The impl lists no
  variants. Ends when quire's derive accepts an empty enum.
- **Two `Percent` types.** `anyview_core::Percent` is an unclamped whole percentage (a volume goes
  to 150); `ds_core::vocab::Percent` clamps to 100. They share no definition because their ranges
  differ. Ends if quire's grows an unclamped sibling.
- **Export defaults and peek budgets are not settings yet.** The JPEG quality (90), the AVIF
  quality (65) and the default paper (A4, portrait) are constants in `export/`; `PeekBudget` has no
  default at all, so the caller reads it from settings. Ends when quire's `22-SETTINGS` has
  `viewer.export.*` and `viewer.peek.*` keys that supply them.
- **The pinned block has no image codecs beyond png and jpeg, and none of the back-end crates.**
  `anyview-image` adds `gif webp bmp tiff ico tga qoi` to the pinned `image` line from its own
  manifest, and `jxl-oxide`, `resvg`, `kamadak-exif`, `img-parts`, `ravif`, `syntect`,
  `pulldown-cmark`, `csv` and `encoding_rs` sit below the pinned block next to `infer`. Ends at the next
  change to quire's `docs/workspace-deps.toml`: add those features to its `image` line and those crates
  to it (CONVENTIONS section 10), then copy the block verbatim here.
- **AVIF decoding is not built or tested here.** The `avif` feature of `anyview-image` needs the dav1d
  C library with its headers and a `dav1d.pc` for pkg-config; this build image has the runtime library
  (`libdav1d7`) but not `libdav1d-dev`, so `cargo clippy --all-features` and `cargo test --all-features`
  stop in `dav1d-sys`. The gate runs without `--all-features`, and an AVIF file is
  `ImageError::NotCompiledIn`. AVIF encoding (`ravif`) is always built and tested. Ends when the image
  or CI has `libdav1d-dev` and `pkg-config`: run the gate with `--all-features`.
- **SVG text is not drawn.** `resvg` is built without its `text` feature and with no font database, and
  external files an SVG names are not read, so only what the SVG itself holds appears. Ends when
  `anyview-platform` can hand the decoder a font database (the `text` feature and `fontdb`).
- **A legacy-encoded text file is not text to the sniffer.** `anyview_core::sniff` calls a head that is
  not valid UTF-8 and has no byte-order mark `Other`, so a Windows-1252 or Shift_JIS `.txt` never reaches
  `anyview-text`; the Windows-1252 fallback of `detect` serves files whose first 4 KiB is ASCII and whose
  accents come later. Ends when `sniff` classifies a head with no NUL and mostly printable bytes as plain
  text under a text extension; no change is needed in `anyview-text`.
- **The only fallback encoding is Windows-1252.** A file in another legacy encoding (Shift_JIS, GBK,
  KOI8-R) is shown as Windows-1252 with mojibake. Ends if that matters: guess with `chardetng`.
- **The back ends' limits are constants.** `PEEK_LINES` (40), `DETECT_BYTES` (1 MiB), the largest image
  to decode (16384 by 16384), the largest animation in memory (512 MiB), the SVG long edge (1024 to 4096),
  the AVIF encoder speed (6), the longest line highlighted (4096 bytes), the largest inlined Markdown
  image (8 MiB) and the top-level keys a fact names (8) are not settings. Ends when quire's `22-SETTINGS`
  has `viewer.peek.*` and `viewer.image.*` keys; the callers then pass them in.
- **The peeks' time budget is not enforced.** `PeekBudget::time` is for the caller: no crate here reads a
  clock, so a slow decode or AVIF encode is abandoned by dropping its worker. Ends if a deadline is
  injected (a `Stamp` source) and the decoders are polled.
- **`CodePeek` builds the highlighter's syntax set on every call.** `Peek::peek` takes no state to keep it
  in, and a global is not allowed. `syntect` loads syntaxes lazily, so a call costs the one syntax it
  uses. Ends if `Peek` gains an environment argument.
- **syntect's default syntaxes lack TypeScript, Kotlin, Swift, TOML, INI, SCSS and Dockerfile.**
  TypeScript is highlighted as JavaScript, Kotlin as Java, SCSS as CSS, and the rest are plain. Ends when
  a bundled syntax pack (Sublime syntax files) is added to `Highlighter::new`.
- **Fenced code in rendered Markdown is highlighted only when the caller passes a `Highlighter`.** The
  `tok-<class>` classes need a stylesheet in the sealed frame that maps them to colour tokens. Ends when
  the Markdown stage's stylesheet lands in `anyview-ui`.
- **JSON is parsed whole and shown as parsed.** A JSON file larger than the peek's byte budget is refused
  with `TextError::JsonOverBudget` (a document cannot be read in part), a JSON Lines file is held in
  memory whole when opened, and a number prints as its value (`1.0` reads `1`). Ends if big JSON matters:
  scan the top level without parsing, and keep numbers as written (`arbitrary_precision`).
- **`TextLines::lines` reads a line whole.** A file that is one enormous line is read into memory when its
  window is asked for. Ends if lines get a byte cap with a marker for the cut.
- **A table is held in memory.** `Table::parse` keeps every row; a very large CSV costs its size several
  times over. Ends if that matters: index record offsets like `TextLines` does and parse windows.
- **Animations loop forever and a peek decodes every frame to count them.** The container's loop count is
  not read, a JPEG XL animation shows its first frame, and the peek's frame count costs a full decode of
  the animation. Ends if any of those hurts: read the GIF and WebP loop counts, and count frames from the
  container headers.
- **`RasterTarget` has no BMP.** `anyview_image::encode_bmp` exists outside it, for callers that need
  the format. Ends if the export sheet offers BMP (the core target and `RasterExportKind` gain it).
- **The history cap and the pruning rule are not settings yet.** `HistoryCap::DEFAULT` is 200
  files, and `record_view` prunes view memory of vanished, replaced and no-longer-listed files
  with no switch. Ends when quire's `22-SETTINGS` has `viewer.history.*` keys; the caller then
  passes the cap to `StoreWriter::new`.
- **History rows carry a `FormatKind`, not a MIME type.** The launcher's file rows are keyed by
  MIME. Ends when `sill-launcher` maps a kind to its MIME (or `anyview-core` exposes the mapping)
  and merges history into file ranking.
- **A touched file forgets its view memory.** The fingerprint is exact length plus modification
  time, so a `touch` or a restore from backup drops the stored position. Ends if that proves
  annoying: compare length only, or add a content hash of the head.
- **`Zoom::Scale` is not clamped by its variant.** Only `Zoom::scaled` clamps to 1% to 6400%; a
  stored or hand-built `Scale(Permille(0))` loads as written. The raster and PDF stages clamp what
  they restore and what they set, so no state of theirs holds an unclamped scale. Ends when `Scale`
  takes a validated scale type.
- **Proposed values of the machines are not settings yet.** The chrome's idle delay
  (`ChromeParams::HIDE_AFTER`, 2000 ms, quire design/20 section 2.6) should read
  `viewer.chrome.hide_after`; the zoom step (x1.25, `viewer.zoom.step`) and the seek step (5 s,
  `viewer.media.seek_step`) likewise. The reveal and hide fades are the motion token `--t-quick`
  (150 ms) and stay tokens. Ends when quire's `22-SETTINGS` has the `viewer.chrome.*`,
  `viewer.zoom.*` and `viewer.media.*` keys; the view then builds `ViewerParams` from them.
- **The viewer's chords bypass `Shortcut::custom`.** ⌘I is the info panel here, and
  `StandardAction::Italic` reserves it, so `Shortcut::custom` would refuse it. `route` matches
  raw `ShortcutKey`s instead. The viewer edits no rich text, so the clash is harmless in the
  window; it matters if a text field in the viewer ever takes ⌘I. Ends when design/27 names ⌘I
  for the viewer or the panel moves to another chord.
- **Which tabs a kind has is the family's, not the profile table's.** `PanelTabs` and `PanelTab` live in
  `anyview-ui`, so `anyview-core`'s `profile` cannot carry a `panel_tabs` column without moving them down.
  `StageView::tabs(doc)` supplies them per document (a PDF's Contents tab exists only when it has an
  outline), and the registry's one match on `FormatKind` supplies the `StageFamily`. Ends if the panel's
  tabs are wanted before a file is open (the launcher's pane): then `PanelTab` moves to `anyview-core` and
  `profile/table.rs` gets the column.
- **A still picture has a first frame only when the shared thumbnail cache has one.** `anyview-image` decodes a
  JPEG or a PNG whole to make even a peek, so a first frame from it would cost what the open costs. The first
  frame of a still is the cache's (`host/pictures.rs`, over the freedesktop cache, large then normal), so only a
  file some program has already thumbnailed has one, and the viewer writes none; a GIF, a WebP or an SVG have
  peeks of `anyview-image`. Ends when `anyview-image` decodes a JPEG at a reduced size, or the viewer stores the
  thumbnail it makes.
- **A page of wrapped text is an estimate.** A line wraps where the stylesheet breaks it; how many lines fit a
  page (the size of a page step, and the lines a window reads) is counted from the width of one glyph of the
  code face (`families/text/wrap.rs`), so a line that breaks a row earlier or later than that makes a page step
  a line short or long. Ends when the renderer can report a laid-out line's height.
- **Home, End and the up and down arrows belong to a text or a PDF while it shows.** They scroll it (the stage is
  asked before navigation is), so such a file is left by the left and right arrows only; every other kind of
  file keeps Home and End for the first and last file. A PDF's Home and End go to the top of its first and last
  page, and a line key moves it a fixed eightieth of a page (`stage/pdf/place.rs`), whatever the zoom. Ends with
  the keymap setting (the stage keys item below).
- **A first frame of a text is only its start.** The first 256 KiB are indexed, so a Markdown file shows as its
  source until the page is rendered, the facts list no line count, a line remembered beyond the start is blank
  until the open lands, and a find waits for the whole file. Ends if a start must be searchable.
- **A search keeps at most 10,000 hits and ignores case only.** `anyview_text::MAX_HITS`; there is no
  whole-word or regular-expression search. Ends when the find bar has options.
- **An animation holds every frame in a texture.** The decode cap (512 MiB of frames) bounds the GPU memory too,
  and an animation near it uploads for as long as it takes to decode. Ends if frames must stream.
- **A rendered Markdown page is a sealed frame that carries the whole design-system stylesheet** (about
  280 KB) in its own document, since a frame inherits nothing; scrolling inside the frame by the wheel is
  Blitz's and untested here. Ends if quire offers the frame's token block alone.
- **The viewer's chords treat Control and Command as one.** `views/keys.rs` folds both to `Super` (⌘), so
  Ctrl+K opens the palette on Linux. A person's own keymap is the settings item above.
- **A zip is not probed.** `sniff` asks for the zip's entries and `anyview-archive` does not exist, so a
  zip-based file fails with `Unsupported` instead of opening as an archive or a document.
- **The launch presentation is not applied.** `Launch` carries no presentation: the window starts as
  `Presentation::Window`, and `Mini` and `Background` need the binary to create the window that way.
- **The capsule's rotate buttons borrow quire's `Undo` and `Refresh` glyphs.** quire has no rotate marks.
  Ends when they are added there (quire FINDINGS).
- **Stage keys are a fixed table.** `StageCommand::from_key` binds `+ = - 0 1 9 v w Space ⇧← ⇧→
  PageUp PageDown ⌘F ⌘G ⇧⌘G`. Ends when the viewer has a keymap setting; the palette shows the
  same keys.
- **Mini stays when the file stops being media.** Walking the sequence from a video in the mini
  window to an image leaves the presentation `Mini`; `Presentation` only leaves `Mini` on
  `ToWindow`. Ends when the root promotes the window as the stage family changes.
- **Rotating a zoomed image refits it.** A centre in the old orientation has no meaning in the
  new one, so `RasterStage` returns to `Fitted` with the new turn. Ends if a rotated view should
  keep its zoom: the centre then needs rotating about the content's middle.
- **Switching a text file between rendered and source keeps the line number.** Rendered lines and
  source lines are not the same lines. Ends when `anyview-text` exposes the mapping.
- **Find hits are addressed by index.** The stages hold the hit count and the current index; the
  edge keeps the hits and maps an index to a place. A document whose hits change while a find is
  open (a reload) must send `Find` again. Ends if live re-search is wanted.
- **`stage_support` is the viewer's current truth.** Raster, vector, Markdown, code, plain text, tables, JSON
  and PDF have a stage (images, text shown as source, PDF pages as tiles); video, audio, fonts, archives,
  books, office documents, folders and unknown files are `PeekOnly`. The registry (`families/registry.rs`)
  maps each `PeekOnly` kind to the facts-and-Open-With… view, and a test holds the two tables equal. Each
  row changes with the stage that lands: video and audio with `anyview-media`, and the registry names the
  new view in the same change.
- **A place is written at most every 500 ms.** Every settled gesture of a stage says `HostRequest::Remember`
  (a PDF's wheel and a text's scroll make dozens a second), so the host keeps the latest place of each file
  (`host/remembering.rs`) and writes it `REMEMBER_EVERY` after the first, on the blocking pool, and what is still
  waiting when the program ends is written by `Hosting::flush`. A place said in the last half second before a
  crash is lost. The interval is a constant until the viewer has settings (a `viewer.*` key).
- **A changed file is told to its window after 150 ms of quiet.** The program's one watcher (`host/watch.rs`,
  `notify`) watches the folder of each window's file, keeps the events that name the file and lets a burst settle
  for `SETTLE` before it calls `Edge::changed`; a file renamed in the viewer is followed, but a folder that is
  removed or unmounted ends the watch without a word. The interval is a constant until the viewer has settings
  (a `viewer.*` key).
- **A PDF link's web address is declined.** `HostRequest::OpenUri` reaches the host, which answers
  `Declined::OpenUri`: `anyview-platform` has no trait for opening an address (the portal's `OpenURI`, or
  `xdg-open`) to carry it out. Ends when it has one.
- **A PDF scrolls by a thousandth of a page.** `PageView::offset` is `Permille` of the page's height, so at a
  high zoom one step is several pixels. The view keeps the exact position itself (`steer.rs`'s `Cursor`) and
  tells the machine only when the page or its offset changes. Ends if the machine takes a finer offset.
- **The PDF panel's thumbnails are drawn near the reader only, and the outline is flat.** Rows exist for
  every page, but a thumbnail is drawn for the 12 pages either side of the reader's (`panel.rs`), because
  the panel has no list that tells the view which rows are on screen; the outline lists every bookmark
  with its depth and ignores `Disclosure`. Ends when quire has a windowed list and the outline a fold.
- **A password-protected PDF is `LoadFailure::Unsupported`.** The viewer has no way to ask for a password;
  `PdfDocument::open_with_password` is there for the sheet that will. Ends with that sheet.
- **pdfrum draws a 1 px blue box round every link annotation,** even when its `/Border` is `[0 0 0]`: the
  tiles of the PDF fixture show it round both links. The links are clickable areas of the view and need no box.
  Ends when pdfrum honours a zero border width or offers a render option without annotations.
- **Legacy and unusual types fall to `Other`.** RAR, JPEG 2000, DjVu, JPEG XR, executables and
  the other types `infer` knows but no family holds are `Other` with `infer`'s media type, shown as
  facts and Open With…. Ends per type when a family holds it.
- **`anyview-platform`'s dependencies sit below the pinned block.** `md-5`, `percent-encoding`, `png` and
  `futures-util` are in no pinned list; `zbus` (with `tokio`), `dirs`, `memfd` and `freedesktop-desktop-entry`
  are. Ends at the next change to quire's `docs/workspace-deps.toml`: add the four and copy the block here.
  `ashpd` is not used: the print portal is called through `zbus` as `ds-blitz`'s `print` does, so the portal
  code exists twice (quire's blocking one and this async one). Ends if `ds-blitz` exposes its portal call or
  the viewer prints through it.
- **No clipboard in `Share`.** The plan's "copy" is the window's clipboard, which must outlive the call and
  belongs to the UI crate; `ShareTarget` has only `Mail` (`xdg-email --attach`). Ends when a freedesktop share
  portal exists, or when the window's clipboard is reached through the platform edge.
- **`WindowStacking` has no Linux protocol.** `NoStacking` answers `Unsupported` for `KeepAbove`: Wayland
  clients cannot place themselves, and COSMIC's protocol is not bound. Ends when `anyview-platform` binds a
  compositor protocol (the shell's layer or a COSMIC toplevel-management request) and the binary passes a
  window handle to `request`.
- **Open With is simple.** `apps_for` matches the exact MIME type: no `mime` subclass or alias (so
  `text/x-rust` is not offered the editors of `text/plain`), no `OnlyShowIn`, `TryExec` or `Terminal=true`,
  entries in subdirectories of `applications/` are found by scan but not by id (`kde-foo.desktop`), names are
  untranslated (`Env` carries no locales), and `%f` receives the path, not a URI. Ends when the shared
  MIME database is read (`shared-mime-info`'s `subclasses` and `aliases`) and `Env` gains locales.
- **A started program is not reaped.** `ProcessSpawn` drops the child handle, so an exited Open With or mail
  program stays a zombie until the viewer exits. Ends when the binary installs a `SIGCHLD` reaper or `Spawn`
  hands the child to a systemd scope.
- **MPRIS `Position` is not announced.** The spec says clients poll it, so `publish` emits no `Seeked`; a
  position jump is visible only on the next read. Ends if a client (sill's now-playing) needs `Seeked`:
  `MediaState` then carries a position-jump marker.
- **The thumbnail cache has no `fail/` directory and no `xx-large`.** A file that cannot be thumbnailed is
  retried every time, and the 1024-pixel size of spec 1.0 is not offered. Ends when the launcher's pane
  measures that retry cost or asks for the size.
- **`file_uri` uses Unix path bytes.** `uri.rs` reads the path as raw bytes (`OsStrExt`), so it does not
  build on Windows. Ends when `windows/` is added: `uri.rs` then encodes through `to_string_lossy` for that
  target.
- **The bus tests need `dbus-daemon`.** The bus tests start one with a private configuration and skip
  with a message on stderr when the program is missing, so a machine without it passes without running them.
  Ends when CI is known to have `dbus-daemon`: make a missing one a failure.
- **Page size is truncated twice.** The rasterizer truncates a page's device size from a float product;
  the scheduler holds sizes in thousandths of a point and plans from one pixel more than it computes, so it
  never leaves a tile out. The render cuts each tile to the real size and leaves out one that starts past
  it, and the scheduler cuts a tile's area to the page as laid out so the spare pixel never makes a tile
  look visible. A scheduled tile that does not exist is therefore silently absent from its batch's result.
- **Search finds within a page's text and a phrase across lines, not across pages.** `search_document`
  loops pages on the caller's worker and extracts each page's text every time; nothing is cached between
  searches. Ends if repeated finds on a large document are slow: keep each page's `TextPage` (or its
  search text) in the worker.
- **A PDF's outline and links keep only pages of the document.** A link to a web address is `Uri`; a named
  action, a script, another file or a page the document lacks is `Other`; a bookmark that leads to none
  of this document's pages has `page: None`. Nothing in the crate opens a URI.
- **Page edits rewrite the whole file, one edit at a time.** `apply` writes the file after each edit and
  opens it again for the next (pdfrum's `reorder_pages` and `delete_pages` number pages as the document was
  opened). A long chain of edits costs one full write each. Ends if that matters: fold the chain into one
  page order, a deletion set and a rotation per page before writing.
- **Rotating a page adds to the turn it has.** `PageOp::Rotate` is relative, as `Edit::Rotate` is; the
  file stores the sum as `/Rotate`. A flip is not an edit a PDF takes (`edits_for(Pdf)` omits it), and
  `page_op` refuses it.
- **Deleting a page drops what points at it.** pdfrum's `delete_pages` removes the page objects with the
  next full save's garbage collection; an outline entry or link that led to a deleted page then leads
  nowhere (`OutlineEntry::page` is `None`, a link is `Other`).

- **The media thread is proven headless, not in a window.** `crates/anyview/tests/media_thread.rs`
  plays through a device made without a surface and samples the picture in its own pass; a real
  window's `TextureLayer` (`Gpu::register_view` of the player's view, `TextureHandle::redraw` from the
  media thread) is not exercised. Ends when the viewer's media view lands: run the same scenario
  against `ds-blitz`'s `Gpu` and keep the test.
- **`Player` hands out a `TextureView`, and its texture is replaced when the slot changes size.**
  `Picture::Shown` is the only way to the picture, so a layer registers it with
  `Gpu::register_view` (wgpu's `TextureView::texture` is not needed), and every `set_slot` to a
  new size makes a new texture that must be registered again. The media actor re-announces the view
  after each size change. Ends if `mpv-wgpu-player` offers a stable texture or a texture-changed event.
- **`mpv-wgpu-player` and `pollster` sit below the pinned block.** They are dev-dependencies of `anyview`
  (the spike), the player at mpv-wgpu `8880898` (master), which resolves the one `wgpu` 29.0.4 the
  tree already has. Ends when the viewer links the player as a normal dependency: add both to quire's
  `docs/workspace-deps.toml` first (CONVENTIONS section 10), then copy the block here.
- **The spike's fixtures are the sibling `mpv-wgpu` checkout's.** `media_thread.rs` reads
  `../mpv/crates/mpv-wgpu-player/tests/fixtures/clip.mkv` and is `#[ignore]` (it needs libmpv, a GPU
  adapter and that checkout). Ends when the media crate carries its own small clip under `tests/fixtures/`.
- **A pool shared by several back ends has one worker scratch per back end per thread.** A back end whose
  scratch is large (pdfrum's `RenderSession` caches) is held by every worker that ran one of its jobs, up
  to the pool size. Ends if memory shows it: give that back end its own smaller `Pool`.
- **A bus activation starts the viewer with no window.** `dist/org.quire.Anyview1.service` runs `anyview` with no
  arguments (its `Exec=/usr/bin/anyview` assumes that install path), and the call that caused it arrives once the
  name is owned. A bus-claiming process with no file now starts its event loop at once with no window
  (`launch_idle`, `LastWindowClosed::StayFor(WARM_FOR)` counting from the start) and each request opens a window
  through the `AppHandle`; a launch with no files and no bus is still a usage error, answered before the loop
  starts. The windows carry the desktop
  application id `org.quire.Anyview`, for which no `.desktop` file is shipped yet: add a `org.quire.Anyview.desktop` entry
  (with `MimeType=` for the families and `Exec=anyview %F`) with the package.
- **Requests the host does not carry out.** `HostRequest` variants and file actions with no effect, each logged as
  "not carried out" (`host::Declined`): `PickFile` (the platform edge has no file chooser), `Export` and the actions
  that open the export sheet (the export pipeline is not wired to the window), `Present` (the mini window and
  background play need window stacking and the player), `Run(CopyFile)` (the clipboard holds text only),
  `Run(Rename)` and `Run(ConvertTo)` (a sheet asks first), `Run(Print)` of anything but a PDF (it prints through an
  export to PDF), `SaveCopy`, `RevertTo`, the flips and a PDF's turns (edits are not wired), `Run(PlayInBackground)` and
  `Peek` and `Play` requests, which open the file like any other until the quick-look window and the player exist.
  Each ends with the feature it waits for.
- **A window is not re-pointed after the host moves its file.** `Rename` moves the file and the host follows it for
  the requests that come after, but the window still shows the old name, and after `Trash` it still shows the
  file. Ends when the views can be told a file moved or went (an input for the load machine and the sequence).
- **Open With opens the default other program.** There is no list to pick from, so `Task::OpenWith` takes the first
  application that handles the type and is not `org.quire.Anyview*`. Ends when the Open With sheet lists
  `AppsForType::apps_for`.
- **Share is mail only and print is a PDF.** `Task::Share` uses the first `ShareTarget` (`Mail`); `Task::Print` hands the
  PDF file's bytes to the portal.
- **A job that panics leaves its load waiting.** The runner delivers a panic as `JobOutcome::Panicked` and the
  program reports it (`seam::notices`), but the `Work` is gone and posts no `Done`, so the window stays on
  its loading state. Ends when `Work` can fail its own ticket (a `Done` for an abandoned job).
- **The thumbnail cache does not paint first.** The platform's `ThumbnailCache` is not looked up at launch: the
  load machine has a `Peeking` state and a `PeekFrame`, and nothing in the window or the binary feeds it. Ends with
  a probe job that answers the cached thumbnail before the open (the views' load flow), and the launch budget's
  first-content time then counts it.
- **The first folder is listed before the first window.** `Opening::around` reads the whole directory (and sorts
  it) of the first file before the window can be built, because `Launch` carries the sequence; on a folder of
  tens of thousands of files that is felt. Ends if `Launch` takes the sequence late (a `Start` input after the
  window is up), as the arrow keys already take it.
- **Appearance, window size, history cap and the store folder are constants.** `Appearance::default()`, 1000 by
  700, `HistoryCap::DEFAULT` and `<data>/anyview` (`program::start`) are not settings. The launcher must read
  the history from the same `<data>/anyview` folder. Ends with quire's `22-SETTINGS` keys.
- **Two tokio runtimes.** The binary's (one worker, `anyview-platform`) is started before the window so the instance
  can be claimed before anything is drawn; ds-blitz's own (two workers) starts with `launch`. Ends if ds-blitz
  can take a runtime handle from the app.
- **The program logs with `eprintln!`.** There is no logging framework in the workspace; every line goes through
  `host::report`, `report_declined` or the few messages in `program::start`, prefixed `anyview:`. Ends when
  quire has a logging path to use.
- **The trash is not behind `anyview-platform`.** `Trash` and `SystemTrash` (the `trash` crate, pure Rust on
  Linux, no bus) live in the binary because the crate is cross-platform. Ends if a platform needs its own: the trait
  then moves to `anyview-platform` beside the others.
- **`trash` is below the pinned block.** It is `anyview`'s alone, MIT (CONVENTIONS section 10): it joins quire's
  `docs/workspace-deps.toml` when sill needs to trash a file.
- **The cold launch is over its budget, and the GPU stack is the reason.** PLAN section 7 sets 150 ms from the
  process starting to the first content of a JPEG cold, and 50 ms warm. Measured 2026-10-03 on the release binary
  with a real window (KWin on Wayland, a 1058 x 618 JPEG, no bus, 8 runs, a machine other builds were also using,
  so +-10 ms; method and the step table in quire's FINDINGS, "Cold start of the first window"): the first frame is
  painted 190 to 200 ms after the process starts, of which `wgpu::Instance::new` (77 ms) and the adapter probe
  across every backend (60 to 68 ms, the GL one can never succeed under the renderer's `display: None`) are 140. The
  picture is ready about 8 ms after the first frame (`tests/launch.rs` below), so first content is near 210 ms.
  With `WGPU_BACKEND=vulkan` in the environment the first frame is at 127 to 150 ms, first content at about
  140 to 160 ms: near the budget, not yet under it every run. quire could not cut it (no `set_var`, the renderer
  reads the backends from the environment only; a thread loading fonts, a GPU warm-up thread and building the
  renderer last were each tried and gained nothing beyond noise). Ends when the renderer takes its backends as an
  option and ds-blitz passes Vulkan on Linux; until then the launcher can set `WGPU_BACKEND=vulkan` (the `.desktop`
  and service `Exec=` lines, `env WGPU_BACKEND=vulkan ...`), which this change does not do: a GL-only machine would
  have no adapter at all.
  Warm: a request forwarded to a viewer with no window open has its window created 30 ms after the
  second launch started (`anyview file` to the compositor's map, 5 ms of it the forwarding), 33 ms with the first frame,
  inside 50 ms; a viewer with a window open and the arrow key to the next picture's first frame is 4.5 to 4.8 ms.
  The bare process (`anyview --help`, exec to exit) is a median 1.1 to 1.4 ms. `crates/anyview/tests/launch.rs`
  (ignored; `cargo test --release -p anyview --test launch -- --ignored --nocapture --test-threads=1`) runs under
  `ds_harness` (no window, no compositor) and holds the ratchet: 300 ms cold, 15 ms warm and 5 ms for the bare
  process, to be lowered whenever a run beats them and never raised. Not measured: a one-page PDF (its stage is not
  in the window yet), the thumbnail cache painting first (the item above).

## Standing facts

- **The `[patch]` sections are quire's, copied.** The root `Cargo.toml` carries quire's `[patch.crates-io]` (the
  vello and anyrender forks) and its `[patch."https://github.com/PoHsuanLai/blitz-kit"]` path entry, because a
  patch applies only at a workspace root and `anyview-ui` now names the render stack through `ds-blitz`. A
  change to either block in quire's root is made here in the same change.

- **`redb` cannot be shared between the viewer and the launcher.** It takes `flock(LOCK_EX)` on
  open and has no shared-reader mode, so the launcher could not read while the viewer ran. The
  store is JSON files replaced by rename instead.
- **A path that is not valid UTF-8 is not remembered.** `StoreWriter::record_view` returns
  `StoreError::PathNotUtf8` before writing anything, because `FilePath` stores through `PathBuf`'s
  serde, which refuses such a path.
- **One writer per store root.** The history is read, changed and rewritten; two writers would
  lose an update and share the `.tmp` name. The viewer is a single instance.

- **The pane and `wgpu`.** The plan says `anyview-peek` reaches none of `wgpu`, but the two things it needs from
  quire, `TextureLayer` (`use_gpu`) and the PDF thumbnail cache (`pdf` feature), both live in `ds-blitz`,
  which depends on `wgpu`, the renderer and pdfrum unconditionally or by feature. So they are in
  `cargo tree -p anyview-peek` and cannot be forbidden there. The boundary is kept where it protects the
  launcher: its tree has no `mpv-wgpu-player`, `rsmpv`, `zbus` or `ashpd` (the script's RULES), its own
  manifest names none of `wgpu`, pdfrum, `tokio`, `anyrender` or the `blitz-*` and `vello` crates (the
  script's DIRECT table), and the pane gets the device only from `ds_blitz::use_gpu`, calling
  `Gpu::device().is_some()` without naming a `wgpu` type. The launcher is a Blitz window on the hybrid
  renderer, so it links all of it already. The dependency budget (`BUDGETS`) is 560 distinct packages;
  `anyview-peek` is 532 today, of which `ds` and `ds-blitz` are nearly all.
- **The `[patch]` sections are copied from quire's and sill's root manifests.** `blitz-kit` points at the
  sibling checkout and the vello and anyrender crates at the `quire-filters` forks, at the revs those
  manifests name; they apply only at a workspace root, so they live in this root. They were added with
  `anyview-peek`, the first crate here that names the renderer, since cargo warns about every unused patch.
- **`Light` is `Peek` plus two conversions.** The plan writes `visit<F: Peek>`; the visitor here takes `P: Light`,
  a supertrait of `Peek` that bounds `Peeked: Into<Body>` and `Error: Into<PeekError>` (associated type
  bounds), so a generic body can turn any peek's result into the one value the pane draws without a
  second match over kinds. Every peek in this crate and in `anyview-image` and `anyview-text` qualifies by
  a blanket impl; a peek that does not has no arm in the registry.
- **`peek` never fails.** A peek that errors is `Body::Unavailable(reason)` with the file's kind, size and
  date, so the launcher always has something to draw; `JsonOverBudget`, an undecodable image and an
  unreadable folder are shown that way. A PDF that pdfrum cannot read is not an error: ds-blitz answers
  with a page that says so (`PdfPage::Failed`), and `PdfThumb` draws the glyph plate.
- **`FormatKind`'s labels are variant names** (`Pdf`, `PlainText` reads `Plain text`), so a folder's kinds read
  `2 plain text, 1 raster` and a facts-only kind with no format shows its media type, not a word. Ends if
  `FormatKind` gets `#[word(label = ..)]` attributes.
- **What sill consumes in phase C.** `anyview_peek::peek` on a worker for the sniffed file (the `Sniffed`
  replaces `Preview`'s `Image`, `TextFile`, `Pdf` and file-facts variants: a picture is `Body::Picture`, a text
  file `Body::Plain` or `Body::Code`, a PDF `Body::Page`, any other file or a folder `Body::FactsOnly` or
  `Body::Folder`), `anyview_peek::Pane` to draw the `Arc<AnyPeeked>` it gets back, and `PeekBudget` from
  its settings. `App`, `Clipboard`, `Emoji` and `Web` stay sill's own. The pane replaces
  `use_pdf_page(path, PANE_MEDIA)` and the image `data:` URLs in
  `sill-launcher-ui/src/launcher/preview/`; see the open item on `PreviewPane` for the frame around it.

- **`infer` has no signature for** TTC fonts, MPEG transport streams, ICNS, TGA, QOI and HDR; the
  extension names them, and only when the head is binary. A text head is never reclassified by a
  binary extension, so an empty `.png` is plain text.
- **`infer`'s `webm` matcher accepts any EBML header**, and its `mkv` matcher (checked first)
  needs `matroska` in the doctype, so Matroska is recognised by magic and everything else EBML is
  WebM.
- **`infer` with default features pulls `cfb`** (legacy Office OLE containers), pure Rust and MIT,
  which is how `.doc`, `.xls` and `.ppt` are recognised by magic.
- **An iWork package and a plain zip look alike by entry names**; the application (Pages, Numbers,
  Keynote) comes from the file's extension, carried in `ZipProbe`. An iWork layout under a `.zip`
  name is an archive.
- **WebP export is lossless only** (`image` has no lossy encoder) and there is no HEIC or JPEG XL
  export: no pure-Rust encoder exists.
- **`Machine::wake` takes no params.** A machine whose timer length is a setting therefore stores
  the instant it waits for (`Shown { hide_at }`), computed with the setting current when the state
  was entered. A changed setting applies from the next step, which is what `Machine::Params`
  documents.
- **`Machine::In: From<Elapsed>` forces an `Elapsed` input on every machine.** The ones with no
  timer list it among their ignored inputs and return `None` from `wake`.
- **`PageCount::new` is not `const`**, so the PDF stage's tests build their params in a function
  rather than a `const`; every other machine's `CASES` is a `const`.

- **`kamadak-exif` reads EXIF but cannot patch it** (its writer builds a block from scratch and would
  drop the maker notes), so `anyview-image` writes the orientation entry itself: in place when the block
  has one, otherwise by copying IFD0 to the end of the block with the entry added, which moves no other
  offset.
- **`img-parts`' JPEG type stops at the end of image and moves the EXIF segment**, so it would drop the
  bytes some cameras append after the image. `rotate_jpeg` therefore scans the file's own markers and
  replaces one APP1 segment; `img-parts` is used only to splice metadata into a freshly encoded file.
- **Decoded pixels are upright, straight alpha.** Orientation is applied by the decoder (the JPEG XL
  decoder applies its own), so nothing downstream reads the tag, and an encode that carries EXIF resets
  its orientation to upright.
- **JPEG has no alpha.** An export to JPEG composites the picture onto white.
- **The image stack matches quire and sill's locks.** `image` 0.25.10, `png`, `gif`, `zune-jpeg`,
  `weezl`, `image-webp`, `resvg` and `usvg` 0.48.1 and `tiny-skia` 0.12.0 resolve to the same versions;
  `thiserror` 2.0.21, `encoding_rs` 0.8.42 and a few others are patch releases newer in this lock. The
  crates added here introduce four duplicates inside this tree: `miniz_oxide` 0.8.9 (`img-parts`) beside
  0.9.1 (`flate2`), `hashbrown` 0.13.2 (`mp4parse`, reached only through the `avif` feature),
  `getrandom` 0.3 and 0.4 and `r-efi` 5 and 6 (`rand` under `rav1e` beside `tempfile`).
- **JPEG XL fixtures are made with ffmpeg's `libjxl`** (`-distance 0`, lossless); `cjxl` is not
  installed. The AVIF fixture exists but is decoded only under the `avif` feature.
- **pdfrum is pinned at 048515cd (PR #101), through quire.** Its `thiserror` and `smallvec` are exact
  pins one patch ahead of quire's older lock, so quire's `Cargo.lock` moved with the pin. The facade is
  built without `edit` and `forms`: page edits and page-range writes go through `pdfrum-edit` directly,
  and the `markdown` feature (plain-layout text and Markdown) is added by `anyview-pdf`'s manifest.
- **Tiles equal the whole page.** At a zoom bucket's scale, a tile drawn with `Region::Rect` is byte for
  byte the same region of a whole-page render on the vello-cpu backend (the test compares them at 72 and
  144 dpi); pdfrum documents an occasional one-count difference on an antialiased edge, which these
  fixtures do not show. A page's device size is the truncated float product, as pdfrum computes it.
- **Page pixels are premultiplied and opaque.** pdfrum's `Pixmap` is premultiplied RGBA8, the form the
  texture layer uploads; a page is drawn on a white background, so alpha is 255 everywhere and straight
  equals premultiplied. `Raster::straight_rgba` is the form `anyview-image`'s encoders take.
- **Zoom buckets are four to the doubling.** Bucket 0 is 1000 permille, every fourth step doubles, the
  ladder runs from 11 to 64000 permille (the viewer's zoom limits), and a shown scale draws at the lowest
  bucket at or above it, so the compositor only scales down.
- **pdfrum's enums are non-exhaustive** (`LinkTarget`, `Error`), and the workspace denies wildcard arms.
  The crate matches the one or two cases it names with `matches!` and `if let` and sends the rest to
  `Other` or `PdfError::Pdf`, rather than writing a wildcard arm.
- **The test fixture is built in memory.** `tests/support/mod.rs` writes a three-page PDF (text on each
  page, an outline of four entries, a link to a page and a link to a web address, a filled rectangle) with
  a correct cross-reference table, about 2.5 KB; no binary fixture is committed.
- **`Player::poll` works from a thread that does not present** (PLAN section 4b, "prove early").
  mpv-wgpu's docs say to call it "on the thread that presents"; the contract that matters is that it
  submits to the `Queue` and writes its own texture, and wgpu's `Device` and `Queue` are `Send + Sync`.
  Checked at mpv-wgpu `8880898` and libmpv 2.5.0 (`pkg-config --modversion mpv`), wgpu 29.0.4, on an
  NVIDIA RTX 5070 Ti (Vulkan): `cargo test -p anyview --test media_thread -- --ignored --nocapture`
  builds the `Device` and `Queue` on the test thread (the window's), builds and polls the `Player`
  only on an `anyview-media` actor thread whose wake is mpv's notify callback, posts each rewritten
  frame's view and a redraw request to the test thread, and samples it in a render pass of the test
  thread, reading it back. Result, five runs and one with `WGPU_ADAPTER_NAME=llvmpipe` (which still
  chose the NVIDIA adapter): 11 frames sampled, 9 distinct, none black, alpha 1 everywhere, every
  poll on one thread that was not the UI thread, a `Seek` sent from the UI thread reached the
  player and frames kept coming (`SeekDone`). So the media architecture of section 4b stands: one
  media thread owns the `Player`, polls it when mpv wakes it, and asks for a redraw; no frame goes
  through the UI thread. The player is built on the actor thread, so it never has to be moved.
- **The runtime adds no dependency.** The pool is `std::thread`, a `Mutex` with a `Condvar` and two
  queues; the mailbox and the actor are a `Mutex` and `std::sync::mpsc`. `crossbeam` and `flume` are in
  the lock only through other crates, and a pool with two lanes and one shared queue needs neither.
- **A raised `Stop` is the flag pdfrum polls.** `Halt` hands pdfrum `Deadline::from_flag(stop.flag())`
  (`Stop::flag` shares the core `Stop`'s own `Arc<AtomicBool>`), with `.with_budget(..)` when the `Stop`
  has a deadline. A raise from any thread ends a draw between the drawn objects of a tile, and pdfrum
  answers `LimitExceeded::Stopped` (or `Time` for the budget), which is `End::Stopped`, never `Failed`.
  pdfrum never lowers a flag, so a `Stop` is one job's: a worker makes a `Halt` per job. Reading a page
  (preparing it) is not interruptible by the session's deadline, so a stop that lands there is noticed
  when the read ends, and a page read cut short is not kept.
- **A worker reads a page once per zoom.** `PdfWorker` keeps the `OwnedPreparedPage` of its last tile
  batch, keyed by (document, page, zoom bucket), and reads again only when that key changes. It holds the
  document's `Arc` through the page; the cache is dropped when the worker is given another document.
  `PdfWorker::pages_prepared` counts the readings, which is how a test sees the reuse.
- **A `TextureLayer` swallows a click.** Blitz hands the pointer to the `<object>` a layer is and goes no
  further up, so a button or row around a picture never hears its click (a press, a move and a drag do reach
  an ancestor). The PDF panel's thumbnails lay a transparent cover over their layer for the click to land on;
  the page tiles need none, because nothing clicks them (a link is an element above them).
- **A scroll gesture carries the modifiers held** (`Gesture::Scroll::held`, quire), which is how the wheel
  under Control zooms the PDF and the wheel alone scrolls it; a pinch zooms about the pointer.
- **A PDF's scale is device pixels per point,** as an image's is device pixels per texel: `Actual` draws a
  point on one device pixel (72 dpi) whatever the screen's density. `Fit` is the whole of the largest page
  inside the room (kept 16 logical pixels from the edges), `Fill` is the widest page across the room's width
  (the capsule's Fit page and Fit width). A zoom between the ladder's steps draws at the next step up and the
  texture is sampled down to size.
- **A PDF's tiles are a bounded cache.** The window holds the textures the workers uploaded under 192 MiB
  (`families/pdf/cache.rs`), letting go first of the zooms farthest from the one on screen and then of the
  pages farthest from the reader, never of a tile the room or its preload margin wants. While a new zoom
  draws, tiles of the old one are drawn under the new ones wherever a new tile is missing (`scene::drawn`).
- **A PDF's jobs borrow renderer scratch from the document.** The job seam gives a worker no state of its
  own (`Work::run` has no worker argument), so `PdfDoc` keeps up to 16 `PdfWorker`s that a job takes and
  returns; the batch of a page that follows a batch of the same page and zoom tends to find the page the
  last one prepared. The binary's `Pool` has its own per-thread scratch for back ends that run through
  `Runner`; moving the PDF to it only changes where `with_scratch` gets its worker.
- **A link opens only what a PDF may ask for.** `HostRequest::OpenUri` is made for `http`, `https` and
  `mailto` addresses; a `file:` link, a script or a named action is ignored, and a link to a page is a jump.
- **PDF work runs in the lane the stage wants it in.** `Work::lane()` (`WorkLane::{Visible, Preload}`, from
  `anyview_pdf::Priority`) is mapped to the pool's `Lane` by `seam/workforce.rs`, so the tiles of the room run
  before the tiles read ahead, thumbnails and links; the stage also ends (`Stop`) the batches a scroll left
  behind.
