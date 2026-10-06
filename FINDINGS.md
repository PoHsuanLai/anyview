# Findings

This file holds two kinds of entry: **open items**, things still unresolved with the condition
that ends each, and **standing facts**, things true at the current pins that code and tests rely
on. It is a reference, not a log: how each was found lives in git history.

## Open items

- **quire's `PdfPage::Ready` carries a PNG `data:` URL.** The PDF peek goes through `ds_blitz::pdf_thumb_blocking`
  (the cache the launcher already uses), whose page is `ImageSource`, so a PDF's first page is the one
  picture here that is not a `TextureLayer`. Ends when quire's `PdfPage::Ready` can hold pixels for a
  `TextureLayer` (a `TextureHandle` or a `Pixels`); then `PdfPeek` hands them over and the pane draws them
  the way it draws any picture.
- **quire's markup lint flags quire's own `TextureLayer`.** The `object.ds-texture-layer` it renders has a
  class no stylesheet rule defines (it styles itself inline), so `ds_lint::markup` reports
  `UnstyledClass` for any page that shows one. `tests/pane.rs` skips exactly that selector. Ends when quire
  gives the class a rule or the lint an allowance for it.
- **A peek's time budget stops only the loops that look at it, and a viewer job stops only between its steps.**
  `PeekBudget::time` is a `Deadline` the folder peek asks about; a decoder that does not return is not
  stopped by it, so the launcher's worker runs each peek on a thread it can abandon (the sill change that
  lands this is `h3`'s `sill.patch`). In the viewer, a load that is left raises its jobs' `Stop` (`Edge`),
  which a queued job honours before it starts and a search honours between batches of lines; an open, an
  unpack of a section and a window of highlighted lines run to their end, and a hung decoder there still
  holds its pool thread. Ends when the decoders take a `Stop` (or run in a plugin process).
- **A peek refuses a picture, font, book or office file over the budget's bytes.** The check is in
  `anyview_peek::peek_with`, ahead of the peeks that read the file whole; a very large camera RAW file shows
  its facts card in the launcher until the image peek reads only the preview it needs.
- **A PDF's page count and title are not known to a peek.** pdfrum is reached only through `ds-blitz`, whose
  thumbnail answers the first page's raster and size; the facts list the page size in points, and the
  Pages and Title rows wait for `anyview-pdf`. A PDF is read whole by `ds-blitz`, so `PdfPeek` refuses one
  longer than the budget's bytes (`PeekError::OverBudget`): the launcher's budget must cover the PDFs it
  wants to show. Ends when `anyview-pdf` supplies a peek that reads only the first page (the registry arm
  names it instead).
- **A modification time is shown in UTC.** `modified_text` reads no zone: this crate has no clock and no
  zone database, and `jiff` is not in its tree. Ends when `anyview-platform` can hand a peek the person's
  zone; the row then moves into `anyview-core`'s `FactValue` with a zone argument.
- **The kinds with no back end show only what sniffing says.** Unknown files
  are `FactsPeek`: the type, the size and the date, with no cover and no listing. Each ends when its crate
  lands and the registry's arm names the real peek. Books are read by `BookPeek` (their cover and facts) and office documents by `OfficePeek` (their facts and the document's own thumbnail). Video and audio are read by pure-Rust header parsers (below).
- **The launcher's media peek is pure Rust, and has gaps a codec library would not.** `anyview-peek`'s `media` feature
  reads headers with `symphonia` (audio), `mp4parse` (MP4, M4V, MOV) and `matroska-demuxer` (MKV, WebM); no
  libav, no libmpv and no `anyview-media` is in its tree. The viewer uses the same parsers for a recording's facts when
  the FFmpeg plugin is not installed. What it cannot do:
  - **AVI, WMV, FLV, MPEG-TS, MPEG and Ogg video get facts only** (type, size, date): nothing here parses them.
  - **A video has a frame only from the desktop's thumbnail cache.** The host passes a `VideoFrames` to
    `peek_with`; with none, or no cached thumbnail for this version of the file, the pane shows the facts card.
  - **A Matroska cover is not read**: `matroska-demuxer` has no attachment accessor. A WebM or MKV shows its
    cached thumbnail or its facts. An MP4's `covr` cover is read.
  - **HEVC is named by scanning the movie box for its sample-entry code**, since `mp4parse` has no codec type
    for it; other codecs it does not know show as `video`. Fragmented MP4 gets its length from `mvhd`, which
    may be zero (no duration row).
  - **The bitrate is the file's size over its length** (less the cover), not the stream's stated rate, and
    `Bitrate` clamps to 32..512 kbit/s, so a lossless file shows 512 at most.
  - **Track lists are counts** (`1 video, 2 audio, 1 subtitles`), only when there is more than one track.
  - **`symphonia` is 0.6**, whose video support is experimental and left off; audio is the stable part.
  - The tree is 590 packages against the budget of 590.
- **An archive listing is bounded by memory and by the budget, not by time.** A zip or a 7z reads its whole
  index inside `PeekBudget::bytes` and is `ArchiveError::OverBudget` past it, so a zip of a hundred thousand
  entries shows "unavailable" until the launcher's budget covers its index. A compressed stream is unpacked
  into memory up to the same number of bytes, so a tarball larger than that lists its first entries with a
  lower-bound count. A 7z's header is checked against the file's length, a count limit (500 000 entries) and
  a byte limit (32 MiB, and 256 MiB of LZMA dictionary) before the 7z crate sees it, an encoded header being
  unpacked inside that limit for the check; `sevenz-rust2` has the same allocate-then-read sites as the crate it
  replaced, so the check is ours. An XLS (whose grid the format limits to 65 536 by 256 cells, about 540 MB of
  cells at the worst) and an ODS are read whole by calamine; XLSX and XLSB are streamed. The bytes of a sheet
  part that hold no cell are bounded by time only. `PeekBudget::time` is not enforced (see the folder item). Ends when a peek has a
  deadline and a listing can stream a tar without holding it.
- **Archive entries are shown as the archive spells them.** A path that is not UTF-8 is lossily converted, a
  zip entry's encoding flag is not consulted (the `zip` crate's own reading decides), and an encrypted entry
  lists normally and is `ArchiveError::Encrypted` only when extracted. RAR is not supported (plan section 3.6a).
- **A font is read whole.** Its tables lie all over the file, so a font longer than the peek budget is
  `FontError::OverBudget` and the pane shows the plate; a large CJK face needs a budget that covers it. Ends
  if the peek reads the table directory and only the tables it needs.
- **The specimen is unshaped.** Each line is set with advance widths: no kerning, no ligatures, no
  right-to-left, no colour glyphs, and a variable font is drawn at its default location. A collection shows
  its first face. A WOFF is unpacked to its sfnt and read; WOFF2 is named, not opened (`face: None`): it needs a
  Brotli decoder and the glyf and loca transforms, and no permissive crate for them is in the tree. Ends
  when one is added to the pinned dependencies and, for shaping, when the specimen lines are drawn by
  parley instead of from outlines.
- **The specimen is fixed text.** The three sample lines are Latin capitals, lowercase and digits; a font
  with none of them shows the first characters it maps. Ends if the pane should show a script's own sample
  (a language of the person's choosing).

- **A folder summary is one level deep.** `FolderPeek` counts the first 10,000 non-hidden entries, adds up the
  size of the files directly inside it (not the contents of folders), and sniffs the first files the byte
  budget pays for (4 KiB each), so on a large folder the named kinds are those of the first files by name.
  Ends if a recursive size matters: it needs a deadline the peeks do not have yet (`PeekBudget::time` is
  not enforced).
- **The pane fills the box its host gives it, but a PDF page is told its size.** `Pane` has no padding of its
  own (the host pads), is as tall as its parent, and its media box (220 px when the parent has no height)
  shrinks so the name and every fact stay inside; a picture, lines, a table and the Markdown frame take
  their size from that box. `PdfThumb` cannot (it draws a page into the `Size` it is given), so `Pane`'s
  `page_room` prop (default `PANE_MEDIA`, 328 by 220 px) is the page's box and the host passes what its box
  leaves. A table still shows at most six columns and forty rows, and code and plain lines are clipped, not
  wrapped or scrolled. Ends when quire's `PdfPage::Ready` becomes a texture the pane draws like a picture.
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
  `anyview-image` adds `gif webp bmp tiff ico tga qoi exr hdr` to the pinned `image` line from its own
  manifest, and `jxl-oxide`, `psd`, `icns`, `resvg`, `kamadak-exif`, `img-parts`, `ravif`, `syntect`,
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
  an `href` that is not a `data:` URL resolves to nothing (`Svg::parse` sets the resolver; usvg's default
  reads any path), so only what the SVG itself holds appears. Ends when
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
- **A JPEG XL animation shows its first frame.** The export sheet exports an animation's first frame, not
  the one on screen. A peek decodes the first frame only and takes the frame count from the container
  (`frame_count.rs`), so a GIF, APNG or WebP whose container lies about its count shows the lie.
- **`RasterTarget` has no BMP.** `anyview_image::encode_bmp` exists outside it, for callers that need
  the format. Ends if the export sheet offers BMP (the core target and `RasterExportKind` gain it).
- **The history cap and the pruning rule are not settings yet.** `HistoryCap::DEFAULT` is 200
  files, and `record_view` prunes view memory of vanished, replaced and no-longer-listed files
  with no switch. Ends when quire's `22-SETTINGS` has `viewer.history.*` keys; the caller then
  passes the cap to `StoreWriter::new`.
- **A history row's media type is its kind's commonest one.** `mime_for(kind)` gives `image/png` for a raster and
  `text/plain` for code and plain text alike, so a launcher row built from the history shows that type's glyph and
  actions, not the file's own media type. The row's pane sniffs the file, so what it shows is right. Ends if the
  history stores the sniffed media type.
- **The pane holds no place, so a handoff brings `Resume::Nothing`.** The launcher's pane shows a file's start and
  scrolls nothing, so opening from it continues where the viewer last left the file. `Handoff` and the wire carry a
  `Resume` for a pane that holds one (a PDF page, a playing position). Ends when the pane has state of its own.
- **A handoff carries no activation token.** `Handoff` (like `Open`) has no platform-data argument, so the window the
  viewer opens for a launcher's handoff relies on the compositor to map it on top. Ends when the compositor
  refuses focus to such a window: the methods then take the xdg-activation token.
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
- **The viewer's own probe does not look inside a zip.** `anyview_peek::probe` does (through
  `anyview_archive::zip_entries`), but `anyview-ui`'s `io/probe.rs` answers `Unrecognised` for a zip, so a
  zip-based file a launcher hands over opens a window that says so, while its pane showed the listing. Ends
  when the probe moves below `anyview-ui` (into a crate both it and `anyview-peek` may name) and the viewer
  calls the same one.
- **The launch presentation is not applied.** `Launch` carries no presentation: the window starts as
  `Presentation::Window`, and `Mini` and `Background` need the binary to create the window that way.
- **The capsule's rotate buttons borrow quire's `Undo` and `Refresh` glyphs.** quire has no rotate marks.
  Ends when they are added there (quire FINDINGS).
- **Stage keys are a fixed table.** `StageCommand::from_key` binds `+ = - 0 1 9 v w Space ⇧← ⇧→
  PageUp PageDown ⌘F ⌘G ⇧⌘G` and, for a recording, `[ ] ⌫ n p a s . , i o` (slower, faster, normal speed,
  next and previous chapter, next audio track, next subtitles, a frame forward and back, the trim's start and
  end). Ends when the viewer has a keymap setting; the palette shows the
  same keys.
- **Mini stays when the file stops being media.** Walking the sequence from a video in the mini
  window to an image leaves the presentation `Mini`; `Presentation` only leaves `Mini` on
  `ToWindow`. Ends when the root promotes the window as the stage family changes.
- **The mini window is a window made again.** A window cannot resize or restyle itself (`HostWindow` has no
  such call), so "Play in Mini Window" opens a new 480 by 270 borderless window for the file, picking up
  the place the old one wrote (the host flushes it first), and closes the old one. The compositor chooses
  where the new one goes. A press on the picture moves it (`begin_move`), so a click on the picture does
  not toggle playback there: the capsule and Space do. Ends when `HostWindow` can set a size and a frame.
- **Keeping the mini window above is asked for and refused.** `Reopen(Mini)` asks `WindowStacking` for
  `KeepAbove` and says once on stderr that the desktop does not allow it (Linux answers `Unsupported`).
  Ends with the platform binding a protocol (see `WindowStacking has no Linux protocol`).
- **Rotating a zoomed image refits it.** A centre in the old orientation has no meaning in the
  new one, so `RasterStage` returns to `Fitted` with the new turn. Ends if a rotated view should
  keep its zoom: the centre then needs rotating about the content's middle.
- **Switching a text file between rendered and source keeps the line number.** Rendered lines and
  source lines are not the same lines. Ends when `anyview-text` exposes the mapping.
- **Find hits are addressed by index.** The stages hold the hit count and the current index; the
  edge keeps the hits and maps an index to a place. A document whose hits change while a find is
  open (a reload) must send `Find` again. Ends if live re-search is wanted.
- **`stage_support` is the viewer's current truth.** Raster, vector, Markdown, code, plain text, tables, JSON,
  PDF, video, audio and books have a stage (images, text shown as source, PDF pages as tiles, a recording as the
  player's picture or an album card, a chapter or comic page in a sealed frame); fonts, archives, office documents, folders and unknown files
  are `PeekOnly`. The registry (`families/registry.rs`) maps each `PeekOnly` kind to the
  facts-and-Open-With… view, and a test holds the two tables equal. Each row changes with the stage that
  lands, and the registry names the new view in the same change.
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
  window handle to `request`. The mini window asks for it and reports the refusal.
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

- **The media thread is proven headless, and the window's texture is fed by it.** `crates/anyview/tests/media_thread.rs`
  runs the real `anyview_media::Driver` on an actor thread of the runtime, builds a device with no
  surface and samples the picture on the test thread in its own render pass (11 frames, 9 distinct, none
  black, every poll on one thread that is not the UI thread). `crates/anyview/tests/media_hub.rs` runs
  `PlayerHost` against a `Gpu` made on such a device: the media thread's sink calls
  `TextureHandle::replace_view` and `redraw`, and the handle's size becomes the player's slot. The window
  itself (`TextureLayer` of that handle) is drawn in the real binary (the `ANYVIEW_SHOTS` pictures of
  the media window were made under the harness with a scripted player; the real binary was run by hand on
  Wayland, see the report of the change that landed it).
- **`Player` hands out a `TextureView`, and its texture is replaced when the slot changes size.**
  `Picture::Shown` is the only way to the picture, so the window's handle shows it with
  `TextureHandle::replace_view`, and every `set_slot` to a new size makes a new texture that must be
  shown again. The driver announces the texture to its `FrameSink` once, and again after each slot
  change. A window that is dragged larger makes a texture for each size it passes through. Ends if
  `mpv-wgpu-player` offers a stable texture or a texture-changed event.
- **`mpv-wgpu-player` and `pollster` sit below the pinned block.** They are `anyview-media`'s, the player at
  mpv-wgpu `d017756` (master) with only its `subprocess` feature (the person's `mpv` as a child process: no
  `rsmpv`, no libmpv), which resolves the one `wgpu` 29.0.4 the tree already has, and neither is shared with quire,
  shell-host or sill, so they stay outside the block like `jxl-oxide` (CONVENTIONS section 10: the block is for
  what the repos share). Ends if another repo links either: they then move into quire's `docs/workspace-deps.toml`
  first.
- **The media crate carries its own fixtures.** `crates/anyview-media/tests/fixtures/` holds `clip.mkv`
  (3 s, mpeg4 64 by 48, two Vorbis tracks, a subtitle track, chapters), `tone.flac` (2 s) and
  `cover.mp3` (2 s with an attached PNG), each under 20 KB, copied from the sibling `mpv-wgpu` checkout's
  player tests. The peek, UI and binary tests reach them by path.
- **cargo-deny sees crates, not programs.** `deny.toml` holds no exception: no crate of the tree binds a codec
  library. Distribution builds of libav and libmpv are GPL (Fedora: `libavcodec-free` GPL-3.0-or-later, `mpv-libs`
  GPL-2.0-or-later), which is why nothing links them: `check-boundary.sh` forbids `rsmpv`, `rsmpv-sys`,
  `ffmpeg-next` and `ffmpeg-sys-next` in every crate and in `Cargo.lock`, and `dev/no-linked-codecs.sh` reads
  `ldd` of the built binary (CONVENTIONS section 15). What the person runs is theirs: mpv, ffmpeg and ffprobe.
- **Which encoders this machine has is the plugin's greeting.** The FFmpeg plugin asks its `ffmpeg -encoders`
  (AAC for M4A, libmp3lame for MP3, FLAC, 16-bit PCM for WAV, libopus or the native Opus for Opus) and lists the
  targets it can write in `hello`; `MediaPlugins::offer` keeps those of the manifest's targets and of the file's
  kind, so the sheet never offers MP3 on a machine with no LAME. Copying a track needs no encoder. Each offer
  starts the plugin once (about 65 ms of `ffmpeg -version` and `-encoders`).
- **A trim is a copy cut at the keyframe at or before its start.** The part kept begins at that keyframe, not
  at the mark (clip.mkv's keyframes are at 0.03, 1.23 and 2.43 s, so a cut from 1.5 s starts at 1.23 s and
  is about 1.27 s long for a mark at 2.5 s), and ends at the first packet past the end mark. It is written
  beside the source as `<name> trimmed.<ext>`; saving a trim in place waits for the save pipeline
  (PLAN phase E).
- **The end of a file mpv holds open is read from the position.** With `keep-open` mpv pauses at the last
  frame and says nothing but `Playback(Paused)`, so the driver calls it ended when the player is paused within a
  quarter second of the length. The child process reports that last position after the pause, so the driver
  looks once a poll's events are all in, and again at the next poll. A pause in the last quarter second is
  therefore read as the end, and Play from there starts again from the start. Ends if the player reports
  `eof-reached`.
- **The child process reports the length and the chapters after the file is loaded.** `Loaded` may carry no
  length; the driver then says `MediaEvent::Length` when mpv does, and the stage takes it (`PlayerEvent::LengthKnown`),
  so for a moment the seek bar has no length. The chapter list arrives the same way: a `GoToChapter` sent
  before it is known is refused. Both are mpv's property events arriving after `file-loaded`; a synchronous read
  would remove the gap and `mpv-wgpu-player` offers none for them. Ends if it does.
- **No position is reported while a seek is in flight.** The position mpv reports between a seek and its
  `playback-restart` is where it was, so the driver holds its reports until `SeekDone` (or `Ended`). It
  is what lets a restored place be applied without the start of the file being reported, and kept, in
  front of it.
- **A recording's place is kept once it has moved a second or changed a setting, and never while opening.**
  `views/arrive.rs` compares what the player reports with what was last kept, and keeps nothing until the
  stage has left `Opening`, so a player's first report cannot overwrite the place a file was left at.
  A place is put back with instructions sent at once, which the driver holds until the file opens.
- **A recording left behind is released, and one ahead is never played.** `StageView::LEAVING` is `Release`
  for the media view: the document the window leaves is dropped (so its player ends) rather than stashed
  for a quick return, and a preload carries no player, so a neighbour recording does not open.
- **A frame is saved only by the window that plays the file.** The sheet lists a frame only for a video with a
  player, but a background session has no window to ask, and a video with no picture track still lists one:
  the player's message is the failure. Ends when a frame is taken from the file without a window (a decode
  plugin).
- **A media export shows no progress and the window cannot stop one.** `ExportRequest::progress` and
  `ExportHandle::stop` exist and are tested, but no view shows a progress or offers Stop, so the desktop task
  reports only how it ended. Ends with the export pipeline's progress sheet (PLAN phase E).
- **The desktop has one now-playing entry for every player.** It shows the session that last started playing,
  and its controls act on that one. Two windows playing at once share it. Next and Previous are not
  offered (`skip` is `Cannot`): walking the sequence is a window's. `Raise` does nothing; `Quit` ends
  the viewer.
- **The volume slider is scaled to 150 percent.** The recording's own level (100) sits at two thirds of the
  slider, since mpv amplifies to 150.
- **The audio driver is `ANYVIEW_AUDIO_OUTPUT`.** `auto` (the default), `pulse`, `pipewire`, `alsa` or `null`,
  read once into `Env`; a name that is none of them is said and ignored. `null` plays nothing: it is how the
  tests and a run on a machine with a person asleep nearby are made quiet.
- **A process that plays with no window stays up.** `Play` (the command line or the bus) starts a player with
  no window, held by `ds_blitz::AppHandle::hold`; stopping it from the now-playing entry (or its recording
  ending) lets the hold go, and the process exits one `WARM_FOR` later if no window opened.
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
- **Requests the host does not carry out.** The few `HostRequest`s and file actions with no effect are logged as
  "not carried out" (`host::Declined`) and, where a person should hear of it, told too (`feedback`): `Present` of a
  quick look or a background session (a window becomes a window or the mini window, nothing offers the others),
  `Run(CopyFile)` (the clipboard of `ds-blitz` holds text only, so the palette hides Copy File until it can hold a
  `text/uri-list`; a patch for quire's `Clipboard` is the way), `Run(Print)` of anything that does not print and the
  edits of a kind that has none. Each ends with the feature it waits for.
- **A renamed file keeps its window and its place.** The host tells the window (`Edge::moved`) and the window opens
  the file again under its new name, where the person is; the folder's list the arrows walk still holds the old
  name until the next listing. After `Trash` the window still shows the file.
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
  700, `HistoryCap::DEFAULT` and `anyview_store::STORE_FOLDER` (`<data>/anyview`, which the launcher reads by the
  same constant) are not settings. Ends with quire's `22-SETTINGS` keys.
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

- **Only the viewer asks the plugins, and the launcher never says `Needs`.** The viewer builds `Plugins` once from
  `Env` and uses them for playing, the facts of a recording and its exports; a recording nothing plays opens as
  its facts with the `Needs: anyview-mpv` row. The launcher's pane (`anyview-peek`) still names its built-in
  readers only, so a kind that will need a plugin (HEIC, RAW) shows its type, size and date there without the
  row. Ends when the peek registry takes the registry and a kind with no back end shows the row.
- **A recording's facts cost a plugin start each time it opens.** `MediaPlugins::reading` runs the FFmpeg plugin's
  probe (about 130 ms of its own and then `ffprobe`) before the player starts, and `offer` starts it again for its
  greeting; opening the same file twice pays both twice. Ends with a cache of the greeting per run, and of the facts
  per file version, if it is measured to matter (it is not yet). Without the plugin the pure-Rust header
  reader answers in a few milliseconds.
- **The viewer's facts for a recording have no length for the seek bar.** `MediaStarted::length` is `None`: the
  plugin's duration is text, and the player says the length itself a moment after the file loads.
- **The FFmpeg plugin was built and tested against FFmpeg 8.1 only** (Fedora's `ffmpeg-free` 8.1.x: aac, libmp3lame, flac,
  pcm, libopus and the experimental native opus are present; no libx264). Plan P3 says FFmpeg 5 to 8: the
  options it passes (`-progress`, `-copypriorss`, `-map 0:V`, `-read_intervals`, the JSON `ffprobe`) exist
  from 4, but no other FFmpeg is installed here to prove it, and the plugin refuses a release older than 4.
  Ends when it runs under a 5 and a 6 or 7 image.
- **A plugin that is killed with SIGKILL leaves its children unless the host kills the group.** Measured:
  ffmpeg keeps running after its `-progress` pipe is closed. `PluginProcess` now starts a plugin in its own
  process group and kills the group on drop; a host that spawns plugins some other way has to do the same.
- **A plugin start costs about 130 ms of its own, then the work.** Measured with this machine's FFmpeg 8.1: `ffmpeg
  -version` and `-encoders` together about 65 ms (ffprobe's own version is not asked), `ffprobe` of a 3 s clip about
  90 ms, one scaled frame about 70 ms. A thumbnail is a start, a probe and a frame: about 280 ms. The thumbnail
  cache hides it for a folder seen before; a first look at 500 recordings would pay 500 starts. This is the measurement the item below waits for:
  if it matters, the session mode (protocol version 2) is the fix, and a cache of the `-encoders` answer is the
  cheaper first step.
- **Trim with several video streams, or a recording whose first keyframe is after the start, aligns to the first
  video stream only.** The keyframe is looked up on the first moving picture; other pictures are copied from the
  same point. A cover or a subtitle that began before the keyframe is dropped.
- **A plugin is started for each request.** Right for the probe, thumbnail and decode of one file and for an
  export, but a folder of 500 recordings pays 500 process starts for its thumbnails. Ends if P3's measurements
  show it: a session mode (kept alive, with request ids announced in `Hello`) is then protocol version 2, and
  version 1 plugins keep working beside it.
- **A decoded picture is one 4-byte-a-pixel copy through a pipe, and one more in the host.** A 24-megapixel
  decode is 96 MB written by the plugin, read in 256 KiB chunks and cut out of the frame buffer (one memmove).
  Measured in the test it is a fraction of a second even in a debug build, so nothing is done about it. Ends if
  a real plugin's decode shows it matters: a memfd handed over a Unix socket (`SCM_RIGHTS`) removes both copies
  and is a new protocol version.
- **The manifest's programs are absolute paths.** The environment has no `PATH` (it comes in through `Env`), so
  a manifest that says `ffmpeg` would be resolved by the process's own, which is the ambient lookup the rules
  forbid. A distribution's package writes its real path. Ends if packaging wants relative names: `Dirs` gains a
  search path and `discover` resolves against it.
- **Only `FactLabel` slugs come back from a probe.** A plugin that knows more than the closed labels (an
  HDR flag, a chapter count) cannot show it; unknown labels are dropped, which is silent: the slugs are kebab-case
  (`audio-codec`, `sample-rate`), and the FFmpeg plugin's tests check that every row it sends parses as a label.
  Ends if a plugin needs a row the labels lack: the label is added to `FactLabel` first, and the plugin sends its slug.
- **The package table is static and names no distribution.** `suggested_package` maps a capability and kind to
  `anyview-ffmpeg` or `anyview-mpv`; Fedora, Debian and Flathub spell them differently or ship them as extensions.
  Ends when packaging exists to say how each names them: the table then takes a distribution from `Env`.
- **Plugin export requests carry a range and a stream and nothing else.** Fields for what P3's exports need
  (a target's bitrate or quality, subtitles, metadata) are added to `ExportRequest` as optional fields, which a
  version 1 plugin ignores. `Plugins` keeps no per-target options either: the manifest lists names only.
- **Playing needs an mpv that loads C plugins, and was tried with Fedora's 0.41 only.** The player starts `mpv
  --script=<mpv-wgpu-cplugin.so>`; Fedora builds `-Dcplugins=enabled`, and Debian 13 (0.40) and 12 (0.35) export the
  render API the plugin needs (mpv-wgpu's spike), but none of those was run for this. An mpv without C plugin
  support fails to start (`PlayerStart`) and the stage says the recording cannot be played; the reason is logged
  and not shown to the person. Ends when CI has a Debian image, and when the stage shows what was missing.
- **The installer writes the path of the mpv it found.** `install.sh --with-plugin mpv` puts the `mpv` on the search
  path (or `--mpv`) into `mpv.toml`, so the viewer never searches the person's `PATH` when it runs; an mpv that
  moves, or is installed later, needs the manifest edited or the installer run again. A packaged `anyview-mpv`
  writes its distribution's path.
- **`dev/media-acceptance.sh` was not run for this change.** It starts the real binary, whose event loop needs
  a display, and the plugins now have to be installed in the scratch data directory (the script writes the mpv
  manifest from `MPV_WGPU_MPV` and `MPV_WGPU_CPLUGIN`). The same checks run against a private bus in
  `crates/anyview/tests/mpris_bus.rs`.
- **The viewer links `anyview-peek` for header facts.** The binary's tree is now the launcher's plus the window:
  664 packages against a budget of 664 (660 before the built-in audio player: see below), with no libmpv or libav in it. Ends if the header readers move to a crate
  of their own that the binary and the peek both link.
- **Audio plays with no mpv, in a built-in player; video and Opus still need mpv.** `anyview-media`'s `audio`
  feature (on for the binary through its own `audio` feature) decodes with symphonia 0.6 and plays through cpal,
  and the binary's `media/engine.rs` picks per recording: mpv when a plugin has it (the built-in player is then
  never asked), else the built-in player when the file is audio, `playable` says its codec has a decoder and its
  first buffer decodes, and the machine has a sound output, else the facts card with `Needs: anyview-mpv (to play
  it)` as before. A machine that could play the file but has no sound output gets `Needs: a sound output (to play
  it)`. It plays MP3, AAC (ADTS, and in M4A), ALAC, FLAC, Ogg Vorbis, and WAV and AIFF PCM. symphonia 0.6.1 has no
  Opus decoder (its codecs are AAC, ADPCM, ALAC, FLAC, MP1/2/3, PCM and Vorbis), so Opus, and any file whose
  picture track makes it a video, stay mpv's. Ends if symphonia gains an Opus decoder: its feature is then added
  to the workspace dependency and `playable` passes it.
- **The built-in player is two threads and one queue.** The media thread (the actor's) decodes, converts and
  pushes into `anyview_media::Pipe`; the sound card's callback drains it, applies the volume and counts the frames
  it took, and wakes the actor as the queue runs low or a twentieth of a second has played. Position is those
  frames, so it is the card's clock: a pause, an underrun and the end are exact whatever the decoder is doing. The
  queue is a `Mutex<VecDeque<f32>>` held for one copy per callback; a lock-free ring would add a package for a
  contention that is a few hundred microseconds a second. `SoundOutput` is the seam: the card is `CardOutput`, the
  tests drain a fake by hand, and `ANYVIEW_AUDIO_OUTPUT=null` is `SilentOutput`, a thread that drains the pipe at
  the recording's pace (the only thread of ours in the crate; every window and hub test plays through it, so no
  test opens a device).
- **cpal 0.17 with no features is four packages.** `cpal`, `alsa`, `alsa-sys` and `dasp_sample` (libc, bitflags and
  cfg-if are already in the tree); 0.18 brings `mach2` 0.6 on other targets and more. The viewer's budget rose from
  660 to 664 for them; `anyview-peek` stays at 590 and the boundary script now forbids `cpal`, `alsa` and
  `alsa-sys` in its tree and `cpal` and `symphonia` in the binary's own manifest. cpal is Apache-2.0, `alsa`
  MIT OR Apache-2.0, `alsa-sys` MIT, `dasp_sample` MIT OR Apache-2.0; `cargo deny check licenses` passes with no
  exception. `alsa-sys` needs libasound's headers to build (Debian `libasound2-dev`, Fedora `alsa-lib-devel`; the CI
  list has it) and the binary links libasound at run time: an audio device library, no codec (`dev/no-linked-codecs.sh`
  passes). PipeWire and PulseAudio answer through libasound's `default` device. `ANYVIEW_AUDIO_OUTPUT` names no
  other host: every name but `null` is the card.
- **What the built-in player does not do.** It plays at the recording's speed (a speed instruction is answered with
  1x), has one track and no chapters, shows no cover (the card is the title and tags), saves no frame, and does
  not step frames. AAC is the low-complexity profile only; a file symphonia marks HE-AAC or USAC is mpv's (untested:
  no encoder here writes one). The sound is converted to the card's format only when the card has no stream at the
  recording's own rate and channels (it asks for those first): a linear resampler and a plain channel mix, which is
  enough for a card that does not follow the file and no substitute for mpv's. Ends if a resampling crate is accepted
  into the budget.
- **Seeking is accurate, and slow where the container cannot seek.** Every seek is symphonia's accurate mode: it
  lands at or before the target and the decoded frames before the target are thrown away, so the first sample after
  a seek is the one asked for (tested on the counter in WAV, AIFF and FLAC to the sample). A container that answers
  with a seek error (ADTS has no index) is read again from its start and decoded up to the target; for an hour-long
  ADTS file that is seconds on the media thread, never on the UI's. Lossy codecs land within a frame of the target.
- **Lengths are the container's.** ADTS AAC and AAC in M4A written without gapless information run the encoder's
  priming and padding long (2.176 s and 2.128 s for 2 s of the fixtures); MP3 with a Xing header and FLAC, WAV, AIFF
  and ALAC are exact. A recording with no stated length plays, says its length when it ends, and has no seek bar
  until then.
- **A decode error stops the recording with "The player stopped", a damaged packet does not.** One packet that fails
  to decode is skipped, as symphonia advises; sixty-four in a row, a read error or a card that fails (unplugged) is
  `MediaEvent::Failed`, which the stage shows as "The player stopped", and nothing more plays. A panic in either
  player is caught at `media/guard.rs` and is the same failure, so it never ends the thread silently.
- **Not checked in tests: the real card.** No test opens a device. What was not seen: sound on real speakers,
  PipeWire's and PulseAudio's behaviour through libasound under a seek or a pause, the media keys and the desktop's
  now-playing widget with the built-in player (the window test presses `MediaControl`s on the fake entry), and a
  card unplugged while playing. Ends with a run by hand on a desktop.
- **A plugin is not sandboxed.** A plugin runs with the person's own rights, as the program
  they installed. The viewer bounds what it will accept (1 MiB of JSON, 512 MiB of pixels, a time limit on
  silence) and nothing else. Ends if plugins come from outside the distribution: a sandbox (bubblewrap, or
  Flatpak's) wraps the spawn.

## Standing facts

- **Spreadsheet and office peeks add `calamine` and `quick-xml`, and the budgets moved for them.** `anyview-text` reads
  XLSX, ODS and XLS through `calamine` (MIT) and `anyview-archive` reads office metadata through `quick-xml`; both are
  outside quire's pinned block. On top of the 600 and 670 the other readers left, they take `anyview-peek` to 605 packages and `anyview` to 675 in
  `scripts/check-boundary.sh`: calamine and quick-xml for XLSX and ODS, with the crates they pull in. A workbook larger than the viewer opens, and a JSON file larger than the peek budget,
  are `Unsupported`, not damaged.
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
  renderer, so it links all of it already. The dependency budget (`BUDGETS`) is 605 distinct packages (raised from 590 for the PSD,
  ICNS and OpenEXR/HDR still formats in `anyview-image`, which the peek links: `psd`, `icns`, `exr` and the
  inflate and SIMD crates `exr` needs, and one more for `roxmltree`, the EPUB package reader, and five more for `calamine` and `quick-xml`, which read XLSX and ODS);
  `anyview-peek` is 605 today, of which `ds` and `ds-blitz` are about 530 and the container codecs of
  `anyview-archive`, `skrifa` and the media parsers the rest.
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
  Checked at mpv-wgpu `8880898` with libmpv 2.5.0 in the process, and again at `d017756` with the child-process
  host (Fedora's stock mpv 0.41 and mpv-wgpu's C plugin: the test passes), wgpu 29.0.4, on an
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
- **The windows follow the desktop's appearance, but the Space look is the root's default.** `Look` carries
  theme, accent, motion, the system's scheme and contrast, and the material and typeface keys; `Ds { look }`
  (a `SpaceLook`) has no key in `appearance.toml`, so it stays the default. A change of the file or the portal
  reaches an open window through `Appearances` and `LookFeed`; the first read of the portal is awaited before
  the first window, so a dark desktop does not flash light.
- **`ds-settings` brings `zbus` to the binary only.** `anyview-ui` takes `Look` as data and does not name
  `ds-settings` (its rule forbids `zbus`); the binary converts `Environment` to `Look` (`host/appearance.rs`).
  `ds-settings`' `tokio` feature is on, matching the pinned `zbus`.
- **`quire/docs/workspace-deps.toml` has drifted from quire's `Cargo.toml`.** The doc still pins blitz-kit
  54b7908 and lacks the fork comment, `proc-macro-crate` and the zbus note; anyview took the doc and moved only
  the blitz-kit rev to quire's 43a6030.
- **`install.sh --set-default` is not undone by `uninstall.sh`.** The default is a line in the person's
  mimeapps.list; a default naming a missing entry is ignored by the desktop. The default covers images, plain
  text, Markdown and PDF, never HTML, tables, audio or video.
- **`tar` is taken from the pinned block with its default features.** A workspace dependency cannot switch
  them off for one member, so `anyview-archive` now also builds `tar`'s `xattr` support (one more package, `xattr`).

- **Books show one section at a time, in a sealed frame.** An EPUB chapter is rebuilt from an allowlist and
  drawn like a Markdown page; a comic page is a `data:` image fitted to the room. Not supported: fixed-layout
  EPUB (it reads as reflowable text), EPUB scripts, audio and video, embedded fonts (`@font-face` is dropped,
  so the system's faces draw), links between chapters (they are inert), CBR (RAR has no permissive decoder),
  encrypted books, and in a comic anything but PNG, JPEG, GIF, WebP and BMP pages. A page is decoded by the
  renderer on the UI thread, so a very large page can hitch a turn. The reading position is the chapter, not
  the scroll inside it. Ends when a section can be scrolled and zoomed by its own machine.

- **Feedback is a toast and a log line, from one seam.** `host/feedback.rs` words every outcome from the task that
  ran (`Doing`) and how it ended (`Outcome`); the window's end of the task asks it. The wording is the host's, the
  drawing the window's (`Done::Notice` to quire's `Toast`), and what the system said stays in the log. An export, a
  duplicate and a copy say the name they took and offer Show in Folder.
- **A launch with no file waits 400 ms for the bus.** A process the bus starts has no file of its own and the
  request that started it arrives just after, so the welcome window opens after `WELCOME_AFTER` unless another window
  has by then; a second launch with no file opens the welcome window at once. Ends when the bus says which kind of
  start it was (`DBUS_STARTER_ADDRESS`, read through `Env`).
- **A row picked in a table or a tree keeps Left and Right.** The arrow keys, Page Up and Down, Home and End move
  the cursor (`RowStep`); while a row is picked Left and Right walk nothing, and Esc puts the cursor away so they
  walk the folder again.
## Saves, versions and the install receipt

- **Save and store limits that stand.** A save keeps the mode, owner (where the process may chown), extended
  attributes and ACLs, and writes a file with other hard links in place so the links stay one file; that write is
  not atomic (the original is kept first). A directory fsync that fails after the rename is
  `Durability::Unconfirmed`, logged by the host. Version listing skips a damaged sidecar silently. The store is
  capped at `DEFAULT_CAP` (2 GiB), pruned oldest first and never a file's newest version; no version is
  pruned by age when the clock is over a year past the newest one. A kept version is not made again when it is
  identical to the file's latest. A save refused for a read-only file is `Io { op: Permissions, kind:
  PermissionDenied }`; `anyview_store::is_read_only` is the query a window uses to disable edits. Rename's no-replace
  falls back to a look-then-rename for a folder on a file system without `RENAME_NOREPLACE` (the one window left).
- **`dist/install.sh` writes a receipt** (`<prefix>/share/anyview/install-receipt`) naming the files it wrote and
  the directories it made; `uninstall.sh` removes exactly those, directories only when empty. An install from
  before the receipt is removed by file names, as it was.
